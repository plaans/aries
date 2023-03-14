use crate::backtrack::{Backtrack, DecLvl};
use crate::core::literals::Disjunction;
use crate::core::state::*;
use crate::core::*;
use crate::model::extensions::{AssignmentExt, DisjunctionExt, SavedAssignment, Shaped};
use crate::model::lang::IAtom;
use crate::model::{Constraint, Label, Model, ModelShape};
use crate::reasoners::{Contradiction, Reasoners};
use crate::reif::{ReifExpr, Reifiable};
use crate::solver::parallel::signals::{InputSignal, InputStream, SolverOutput, Synchro};
use crate::solver::search::{default_brancher, Decision, SearchControl};
use crate::solver::stats::Stats;
use crate::utils::cpu_time::StartCycleCount;
use crossbeam_channel::Sender;
use env_param::EnvParam;
use std::fmt::Formatter;
use std::sync::Arc;
use std::time::Instant;

/// If true, decisions will be logged to the standard output.
static LOG_DECISIONS: EnvParam<bool> = EnvParam::new("ARIES_LOG_DECISIONS", "false");

/// Macro that uses the the same syntax as `println!()` but:
///  - only evaluate arguments and print if `LOG_DECISIONS` is true.
///  - prepends the thread id to the line.
macro_rules! log_dec {
    // log_dec!("a {} event", "log")
    ($($arg:tt)+) => {
        if LOG_DECISIONS.get() {
            print!("[{:?}] ", std::thread::current().id());
            println!($($arg)+);
        }
    }
}

/// Result of the `_solve` method.
enum SolveResult {
    /// A solution was found through search and the solver's assignment is on this solution
    AtSolution,
    /// The solver was made aware of a solution from its input channel.
    ExternalSolution(Arc<SavedAssignment>),
    /// The solver has exhausted its search space.
    Unsat,
}

#[derive(Debug)]
pub enum Exit {
    Interrupted,
}
impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Solver interrupted.")
    }
}
impl std::error::Error for Exit {}

pub struct Solver<Lbl> {
    pub model: Model<Lbl>,
    /// Index of the next constraint to post in the model.
    next_unposted_constraint: usize,
    pub brancher: Box<dyn SearchControl<Lbl> + Send>,
    pub reasoners: Reasoners,
    decision_level: DecLvl,
    pub stats: Stats,
    /// A data structure with the various communication channels
    /// needed to receive/send updates and commands.
    sync: Synchro,
}
impl<Lbl: Label> Solver<Lbl> {
    pub fn new(model: Model<Lbl>) -> Solver<Lbl> {
        Solver {
            model,
            next_unposted_constraint: 0,
            brancher: default_brancher(),
            reasoners: Reasoners::new(),
            decision_level: DecLvl::ROOT,
            stats: Default::default(),
            sync: Synchro::new(),
        }
    }

    pub fn set_brancher(&mut self, brancher: impl SearchControl<Lbl> + 'static + Send) {
        self.brancher = Box::new(brancher)
    }

    pub fn set_brancher_boxed(&mut self, brancher: Box<dyn SearchControl<Lbl> + 'static + Send>) {
        self.brancher = brancher
    }

    pub fn input_stream(&self) -> InputStream {
        self.sync.input_stream()
    }

    pub fn set_solver_output(&mut self, output: Sender<SolverOutput>) {
        self.sync.set_output(output);
    }

    pub fn enforce<Expr: Reifiable<Lbl>>(&mut self, bool_expr: Expr, scope: impl IntoIterator<Item = Lit>) {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        self.model.enforce(bool_expr, scope);
    }
    pub fn enforce_all<Expr: Reifiable<Lbl>>(
        &mut self,
        bools: impl IntoIterator<Item = Expr>,
        scope: impl IntoIterator<Item = Lit> + Clone,
    ) {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        self.model.enforce_all(bools, scope);
    }

    /// Immediately adds the given constraint to the appropriate reasoner.
    /// Returns an error if the model become invalid as a result.
    fn post_constraint(&mut self, constraint: &Constraint) -> Result<(), InvalidUpdate> {
        let Constraint::Reified(expr, value) = constraint;
        let value = *value;
        assert_eq!(self.model.state.current_decision_level(), DecLvl::ROOT);
        let scope = self.model.presence_literal(value.variable());
        match expr {
            &ReifExpr::Lit(lit) => {
                let expr_scope = self.model.presence_literal(lit.variable());
                assert!(self.model.state.implies(scope, expr_scope), "Incompatible scopes");
                self.add_clause([!value, lit], scope)?; // value => lit
                self.add_clause([!lit, value], scope)?; // lit => value
                Ok(())
            }
            ReifExpr::MaxDiff(diff) => {
                let rhs = diff.a;
                let rhs_add = diff.ub;
                let lhs = diff.b;
                self.reasoners
                    .diff
                    .add_reified_edge(value, rhs, lhs, rhs_add, &self.model.state);
                Ok(())
            }
            ReifExpr::Or(disjuncts) => {
                if self.model.entails(value) {
                    self.add_clause(disjuncts, scope)
                } else if self.model.entails(!value) {
                    // (not (or a b ...))
                    // enforce the equivalent (and (not a) (not b) ....)
                    for &lit in disjuncts {
                        self.add_clause([!lit], scope)?;
                    }
                    Ok(())
                } else {
                    // l  <=>  (or a b ...)
                    let mut clause = Vec::with_capacity(disjuncts.len() + 1);
                    // make l => (or a b ...)    <=>   (or (not l) a b ...)
                    clause.push(!value);
                    disjuncts.iter().for_each(|l| clause.push(*l));
                    if let Some(clause) = Disjunction::new_non_tautological(clause) {
                        self.add_clause(clause, scope)?;
                    }
                    // make (or a b ...) => l    <=> (and (a => l) (b => l) ...)
                    for &disjunct in disjuncts {
                        // enforce a => l
                        self.add_clause([!disjunct, value], scope)?;
                    }
                    Ok(())
                }
            }
            ReifExpr::And(_) => {
                let equiv = Constraint::Reified(!expr.clone(), !value);
                self.post_constraint(&equiv)
            }
            ReifExpr::Linear(lin) => {
                assert!(self.model.entails(value), "Unsupported reified linear constraints.");
                assert_eq!(self.model.presence_literal(value.variable()), Lit::TRUE);
                self.reasoners.cp.add_linear_constraint(lin);
                Ok(())
            }
        }
    }

    /// Adds a disjunctive constraint within the given scope.
    fn add_clause(&mut self, clause: impl Into<Disjunction>, scope: Lit) -> Result<(), InvalidUpdate> {
        assert_eq!(self.current_decision_level(), DecLvl::ROOT);
        let clause = clause.into();
        // only keep literals that may become true
        let clause: Vec<Lit> = clause.into_iter().filter(|&l| !self.model.entails(!l)).collect();
        let (propagatable, scope) = self.scoped_disjunction(clause, scope);
        if propagatable.is_empty() {
            return self.model.state.set(!scope, Cause::Encoding).map(|_| ());
        }
        self.reasoners.sat.add_clause_scoped(propagatable, scope);
        Ok(())
    }

    /// From a disjunction with optional elements, creates a scoped clause that can be safely unit propagated
    /// TODO: generalize to also look at literals in the clause as potential scopes
    pub(in crate::solver) fn scoped_disjunction(
        &self,
        disjuncts: impl Into<Disjunction>,
        scope: Lit,
    ) -> (Disjunction, Lit) {
        let prez = |l: Lit| self.model.presence_literal(l.variable());
        // let optional = |l: Lit| prez(l) == Lit::TRUE;
        let disjuncts = disjuncts.into();
        if scope == Lit::TRUE {
            return (disjuncts, scope);
        }
        if disjuncts.is_empty() {
            // the disjunction can never be true and thus must be absent
            return (Disjunction::from([!scope]), Lit::TRUE);
        }
        if disjuncts
            .literals()
            .iter()
            .all(|&l| self.model.state.implies(prez(l), scope))
        {
            return (disjuncts, scope);
        }
        let mut disjuncts = Vec::from(disjuncts);
        disjuncts.push(!scope);

        (disjuncts.into(), Lit::TRUE)
    }

    /// Post all constraints of the model that have not been previously posted.
    fn post_constraints(&mut self) -> Result<(), InvalidUpdate> {
        if self.next_unposted_constraint == self.model.shape.constraints.len() {
            return Ok(()); // fast path that avoids updating metrics
        }
        let start_time = Instant::now();
        let start_cycles = StartCycleCount::now();
        while self.next_unposted_constraint < self.model.shape.constraints.len() {
            let c = &self.model.shape.constraints[self.next_unposted_constraint].clone();
            self.post_constraint(c)?;
            self.next_unposted_constraint += 1;
        }
        self.stats.init_time += start_time.elapsed();
        self.stats.init_cycles += start_cycles.elapsed();
        Ok(())
    }

    /// Searches for the first satisfying assignment, returning none if the search
    /// space was exhausted without encountering a solution.
    pub fn solve(&mut self) -> Result<Option<Arc<SavedAssignment>>, Exit> {
        match self._solve()? {
            SolveResult::AtSolution => Ok(Some(Arc::new(self.model.state.clone()))),
            SolveResult::ExternalSolution(s) => Ok(Some(s)),
            SolveResult::Unsat => Ok(None),
        }
    }

    /// Implementation of the public facing `solve()` method that provides more control.
    /// In particular, the output distinguishes between whether the solution was found by this
    /// solver or another one (i.e. was read from the input channel).
    fn _solve(&mut self) -> Result<SolveResult, Exit> {
        // make sure brancher has knowledge of all variables.
        self.brancher.import_vars(&self.model);

        let start_time = Instant::now();
        let start_cycles = StartCycleCount::now();
        loop {
            while let Ok(signal) = self.sync.signals.try_recv() {
                match signal {
                    InputSignal::Interrupt => {
                        self.stats.solve_time += start_time.elapsed();
                        self.stats.solve_cycles += start_cycles.elapsed();
                        return Err(Exit::Interrupted);
                    }
                    InputSignal::LearnedClause(cl) => {
                        self.reasoners.sat.add_forgettable_clause(cl.as_ref());
                    }
                    InputSignal::SolutionFound(assignment) => {
                        self.stats.solve_time += start_time.elapsed();
                        self.stats.solve_cycles += start_cycles.elapsed();
                        return Ok(SolveResult::ExternalSolution(assignment));
                    }
                }
            }

            if !self.propagate_and_backtrack_to_consistent() {
                // UNSAT
                self.stats.solve_time += start_time.elapsed();
                self.stats.solve_cycles += start_cycles.elapsed();
                return Ok(SolveResult::Unsat);
            }
            match self.brancher.next_decision(&self.stats, &self.model) {
                Some(Decision::SetLiteral(lit)) => {
                    // println!("Decision: {}", self.model.fmt(lit));
                    self.decide(lit);
                }
                Some(Decision::Restart) => {
                    self.reset();
                    self.stats.add_restart();
                }
                None => {
                    log_dec!("=> SOLUTION");
                    // SAT: consistent + no choices left
                    self.stats.solve_time += start_time.elapsed();
                    self.stats.solve_cycles += start_cycles.elapsed();
                    debug_assert!({
                        self.model.shape.validate(&self.model.state).unwrap();
                        true
                    });
                    return Ok(SolveResult::AtSolution);
                }
            }
        }
    }

    pub fn minimize(&mut self, objective: impl Into<IAtom>) -> Result<Option<(IntCst, Arc<SavedAssignment>)>, Exit> {
        self.minimize_with(objective, |_, _| ())
    }

    pub fn minimize_with(
        &mut self,
        objective: impl Into<IAtom>,
        on_new_solution: impl FnMut(IntCst, &SavedAssignment),
    ) -> Result<Option<(IntCst, Arc<SavedAssignment>)>, Exit> {
        self.optimize_with(objective.into(), true, on_new_solution)
    }

    pub fn maximize(&mut self, objective: impl Into<IAtom>) -> Result<Option<(IntCst, Arc<SavedAssignment>)>, Exit> {
        self.maximize_with(objective, |_, _| ())
    }

    pub fn maximize_with(
        &mut self,
        objective: impl Into<IAtom>,
        on_new_solution: impl FnMut(IntCst, &SavedAssignment),
    ) -> Result<Option<(IntCst, Arc<SavedAssignment>)>, Exit> {
        self.optimize_with(objective.into(), false, on_new_solution)
    }

    fn optimize_with(
        &mut self,
        objective: IAtom,
        minimize: bool,
        mut on_new_solution: impl FnMut(IntCst, &SavedAssignment),
    ) -> Result<Option<(IntCst, Arc<SavedAssignment>)>, Exit> {
        // best solution found so far
        let mut best = None;
        loop {
            let sol = match self._solve()? {
                SolveResult::AtSolution => {
                    // solver stopped at a solution, this is necessarily an improvement on the best solution found so far
                    let sol = Arc::new(self.model.state.clone());
                    // notify other solvers that we have found a new solution
                    self.sync.notify_solution_found(sol.clone());
                    let objective_value = sol.var_domain(objective).lb;
                    on_new_solution(objective_value, &sol);
                    sol
                }
                SolveResult::ExternalSolution(sol) => sol, // a solution was handed out to us by another solver
                SolveResult::Unsat => return Ok(best), // exhausted search space, return the best result found so far
            };

            // determine whether the solution found is an improvement on the previous one (might not be the case if sent by another solver)
            let objective_value = sol.var_domain(objective).lb;
            let is_improvement = match best {
                None => true,
                Some((previous_best, _)) => {
                    if minimize {
                        objective_value < previous_best
                    } else {
                        objective_value > previous_best
                    }
                }
            };

            if is_improvement {
                // Notify the brancher that a new solution has been found.
                // This enables the use of LNS-like solution and letting the brancher use the values in the best solution
                // as the preferred ones.
                self.brancher.new_assignment_found(objective_value, sol.clone());
                self.stats.add_solution(objective_value); // TODO: might consider external solutions
                                                          // save the best solution
                best = Some((objective_value, sol));

                // restart at root with a constraint enforcing future solution to improve the objective
                self.reset();
                if minimize {
                    // println!("Setting objective < {objective_value}");
                    self.reasoners.sat.add_clause([objective.lt_lit(objective_value)]);
                } else {
                    // println!("Setting objective > {objective_value}");
                    self.reasoners.sat.add_clause([objective.gt_lit(objective_value)]);
                }
            }
        }
    }

    pub fn decide(&mut self, decision: Lit) {
        self.save_state();
        log_dec!(
            "decision: {:?} -- {}     dom:{:?}",
            self.decision_level,
            self.model.fmt(decision),
            self.model.domain_of(decision.variable())
        );
        let res = self.model.state.decide(decision);
        assert_eq!(res, Ok(true), "Decision did not result in a valid modification.");
        self.stats.add_decision(decision)
    }

    /// Determines the appropriate backtrack level for this clause and returns the literal that
    /// is asserted at this level.
    ///
    /// The common understanding is that it should be the earliest level at which the clause is unit.
    /// However, the explanation of eager propagation of optional might generate explanations where some
    /// literals are not violated, those are ignored in determining the asserting level.
    ///
    /// If there is more that one violated literal at the latest level, then no literal is asserted
    /// and the bactrack level is set to the ante-last level (might occur with clause sharing).
    fn backtrack_level_for_clause(&self, clause: &[Lit]) -> Option<(DecLvl, Option<Lit>)> {
        // debug_assert_eq!(self.model.state.value_of_clause(clause.iter().copied()), Some(false));

        // level of the the two latest set element of the clause
        let mut max = DecLvl::ROOT;
        let mut max_next = DecLvl::ROOT;

        // the latest violated literal, which will be the asserted literal.
        let mut asserted = None;

        for &lit in clause {
            // only consider literals that are violated.
            // non violated literals might be there because of eager propagation of optionals.
            if self.model.state.entails(!lit) {
                if let Some(ev) = self.model.state.implying_event(!lit) {
                    let dl = self.model.state.trail().decision_level(ev);
                    if dl > max {
                        max_next = max;
                        max = dl;
                        asserted = Some(lit);
                    } else if dl > max_next {
                        max_next = dl;
                    }
                }
            }
        }

        if max == DecLvl::ROOT {
            None
        } else if max == max_next {
            Some((max - 1, None))
        } else {
            assert!(max_next < max);
            Some((max_next, asserted))
        }
    }

    /// Integrates a conflicting clause (typically learnt through conflict analysis)
    /// and backtracks to the appropriate level.
    /// As a side effect, the activity of the variables in the clause will be increased.
    /// Returns `false` if the clause is conflicting at the root and thus constitutes a contradiction.
    #[must_use]
    fn add_conflicting_clause_and_backtrack(&mut self, expl: Conflict) -> bool {
        // // print the clause before analysis
        // println!("conflict ({}) :", expl.literals().len());
        // for &l in expl.literals() {
        //     if !self.model.state.entails(!l) {
        //         print!("  > {}", self.model.fmt(l));
        //     } else {
        //         print!("    {}", self.model.fmt(l));
        //     }
        //     // let prez = self.model.state.presence(l.variable());
        //     // let scope: Vec<Lit> = self
        //     //     .model
        //     //     .state
        //     //     .implications
        //     //     .direct_implications_of(l.variable().geq(1))
        //     //     .collect();
        //     // print!("  / {}   <<<<   ", self.model.fmt(prez));
        //     // print!("  [");
        //     // for prez in scope {
        //     //     print!("  & {}", self.model.fmt(prez));
        //     // }
        //     // println!("]");
        // }
        // println!();

        if let Some((dl, asserted)) = self.backtrack_level_for_clause(expl.literals()) {
            // inform the brancher that we are in a conflict state
            self.brancher.conflict(&expl, &self.model, &mut self.reasoners);
            // backtrack
            self.restore(dl);
            debug_assert_eq!(self.model.state.value_of_clause(&expl.clause), None);

            if let Some(asserted) = asserted {
                // add clause to sat solver, making sure the asserted literal is set to true
                self.reasoners.sat.add_learnt_clause(expl.clause, asserted);
                self.brancher.asserted_after_conflict(asserted, &self.model)
            } else {
                // no asserted literal, just add a forgettable clause
                self.reasoners.sat.add_forgettable_clause(expl.clause)
            }

            true
        } else {
            false
        }
    }

    /// Propagate all constraints until reaching a consistent state or proving that there is no such
    /// consistent state (i.e. the problem is UNSAT).
    ///
    /// This will be done by:
    ///  - propagating in the current state
    ///    - return if no conflict was detected
    ///    - otherwise: learn a conflicting clause, backtrack up the decision tree and repeat the process.
    #[must_use]
    pub fn propagate_and_backtrack_to_consistent(&mut self) -> bool {
        loop {
            match self.propagate() {
                Ok(()) => return true,
                Err(conflict) => {
                    log_dec!(
                        " CONFLICT {:?} (size: {}) ",
                        self.decision_level,
                        conflict.clause.len(),
                        // conflict.literals().iter().map(|l| self.model.fmt(*l)).format(", ")
                    );
                    self.sync.notify_learnt(&conflict.clause);
                    if self.add_conflicting_clause_and_backtrack(conflict) {
                        // we backtracked, loop again to propagate
                    } else {
                        // could not backtrack to a non-conflicting state, UNSAT
                        return false;
                    }
                }
            }
        }
    }

    /// Fully propagate all constraints until quiescence or a conflict is reached.
    ///
    /// Returns:
    /// - `Ok(())`: if quiescence was reached without finding any conflict
    /// - `Err(clause)`: if a conflict was found. In this case, `clause` is a conflicting cause in the current
    ///   decision level that   
    pub fn propagate(&mut self) -> Result<(), Conflict> {
        match self.post_constraints() {
            Ok(()) => {}
            Err(_) => {
                assert_eq!(self.current_decision_level(), DecLvl::ROOT);
                return Err(Conflict::contradiction());
            }
        }
        let global_start = StartCycleCount::now();

        // we might need to do several rounds of propagation to make sur the first inference engines,
        // can react to the deductions of the latest engines.
        loop {
            let num_events_at_start = self.model.state.num_events();

            // propagate all theories
            for &i in self.reasoners.writers() {
                let theory_propagation_start = StartCycleCount::now();
                self.stats[i].propagation_loops += 1;
                let th = self.reasoners.reasoner_mut(i);

                match th.propagate(&mut self.model.state) {
                    Ok(()) => (),
                    Err(contradiction) => {
                        self.brancher.pre_conflict_analysis(&self.model);
                        // contradiction, learn clause and exit
                        let clause = match contradiction {
                            Contradiction::InvalidUpdate(fail) => {
                                self.model.state.clause_for_invalid_update(fail, &mut self.reasoners)
                            }
                            Contradiction::Explanation(expl) => {
                                self.model.state.refine_explanation(expl, &mut self.reasoners)
                            }
                        };
                        self.stats.add_conflict(self.current_decision_level(), clause.len());
                        self.stats[i].conflicts += 1;
                        self.stats.propagation_time += global_start.elapsed();
                        self.stats[i].propagation_time += theory_propagation_start.elapsed();
                        return Err(clause);
                    }
                }
                self.stats[i].propagation_time += theory_propagation_start.elapsed();
            }

            if num_events_at_start == self.model.state.num_events() {
                // no new events, inferred in this propagation loop, exit.
                break;
            }
        }
        self.stats.propagation_time += global_start.elapsed();
        Ok(())
    }

    pub fn print_stats(&self) {
        println!("{}", self.stats);
        for (i, th) in self.reasoners.theories() {
            println!("====== {i} =====");
            th.print_stats();
        }
    }
}

impl<Lbl> Backtrack for Solver<Lbl> {
    fn save_state(&mut self) -> DecLvl {
        self.brancher.pre_save_state(&self.model);
        self.decision_level += 1;
        let n = self.decision_level;
        assert_eq!(self.model.save_state(), n);
        assert_eq!(self.brancher.save_state(), n);

        for w in self.reasoners.writers() {
            let th = self.reasoners.reasoner_mut(*w);
            assert_eq!(th.save_state(), n);
        }
        n
    }

    fn num_saved(&self) -> u32 {
        debug_assert!({
            let n = self.decision_level.to_int();
            assert_eq!(self.model.num_saved(), n);
            assert_eq!(self.brancher.num_saved(), n);
            for (_, th) in self.reasoners.theories() {
                assert_eq!(th.num_saved(), n);
            }
            true
        });
        self.decision_level.to_int()
    }

    fn restore_last(&mut self) {
        assert!(self.decision_level > DecLvl::ROOT);
        self.restore(self.decision_level - 1);
        self.decision_level -= 1;
    }

    fn restore(&mut self, saved_id: DecLvl) {
        self.decision_level = saved_id;
        self.model.restore(saved_id);
        self.brancher.restore(saved_id);
        for w in self.reasoners.writers() {
            let th = self.reasoners.reasoner_mut(*w);
            th.restore(saved_id);
        }
        debug_assert_eq!(self.current_decision_level(), saved_id);
    }
}

impl<Lbl: Label> Clone for Solver<Lbl> {
    fn clone(&self) -> Self {
        Solver {
            model: self.model.clone(),
            next_unposted_constraint: self.next_unposted_constraint,
            brancher: self.brancher.clone_to_box(),
            reasoners: self.reasoners.clone(),
            decision_level: self.decision_level,
            stats: self.stats.clone(),
            sync: self.sync.clone(),
        }
    }
}

impl<Lbl: Label> Shaped<Lbl> for Solver<Lbl> {
    fn get_shape(&self) -> &ModelShape<Lbl> {
        self.model.get_shape()
    }
}

#[cfg(test)]
mod test {
    use crate::core::literals::Disjunction;
    use crate::core::Lit;

    type Model = crate::model::Model<&'static str>;
    type Solver = crate::solver::Solver<&'static str>;

    #[test]
    fn test_scoped_disjunction() {
        let mut m = Model::new();

        let px = m.new_presence_variable(Lit::TRUE, "px").true_lit();
        let x1 = m.new_optional_bvar(px, "x1").true_lit();
        let x2 = m.new_optional_bvar(px, "x2").true_lit();

        let py = m.new_presence_variable(Lit::TRUE, "py").true_lit();
        // let y1 = m.new_optional_bvar(py, "y1").true_lit();
        // let y2 = m.new_optional_bvar(py, "y2").true_lit();

        let pxy = m.get_conjunctive_scope(&[px, py]);
        let xy1 = m.new_optional_bvar(pxy, "xy1").true_lit();
        // let xy2 = m.new_optional_bvar(pxy, "xy2").true_lit();

        let s = &Solver::new(m);

        fn check(
            s: &Solver,
            scope: Lit,
            clause: impl Into<Disjunction>,
            expected: impl Into<Disjunction>,
            expected_scope: Lit,
        ) {
            let clause = clause.into();
            let result = s.scoped_disjunction(clause, scope);
            let expected = expected.into();
            assert_eq!(result, (expected, expected_scope));
        }

        check(s, px, [x1], [x1], px);
        // check(s, T, [!px, x1], [x1]);
        check(s, px, [x1, x2], [x1, x2], px);
        // check(s, T, [!px, x1, x2], [x1, x2]);
        check(s, px, [xy1], [xy1], px); // ??
        check(s, py, [xy1], [xy1], py);
        check(s, pxy, [xy1], [xy1], pxy);
        check(s, pxy, [x1], [!pxy, x1], Lit::TRUE);
        // check(s, T, [!pxy, xy1], [xy1]);
        // check(s, T, [!px, !py, xy1], [xy1]);
        // check(s, T, [!px, !py], [!px, !py]); // !pxy, would be correct as well
    }
}
