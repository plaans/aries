use crossbeam_channel::Sender;
use itertools::Itertools;
use std::fmt::Formatter;
use std::sync::Arc;
use std::time::Instant;

use aries_backtrack::{Backtrack, DecLvl};
use aries_core::literals::Disjunction;
use aries_core::state::*;
use aries_core::*;
use aries_model::decomposition::Constraints;
use aries_model::extensions::{AssignmentExt, DisjunctionExt, SavedAssignment, Shaped};
use aries_model::lang::expr::Normalize;
use aries_model::lang::reification::{BindTarget, ReifiableExpr};
use aries_model::lang::IAtom;
use aries_model::{Label, Model, ModelShape};
use env_param::EnvParam;

use crate::cpu_time::CycleCount;
use crate::cpu_time::StartCycleCount;
use crate::signals::{InputSignal, InputStream, SolverOutput, Synchro};
use crate::solver::sat_solver::SatSolver;
use crate::solver::search::{default_brancher, Decision, SearchControl};
use crate::solver::stats::Stats;
use crate::solver::theory_solver::TheorySolver;
use crate::{Bind, Contradiction, Theory};

pub mod sat_solver;
pub mod search;
pub mod stats;
pub mod theory_solver;

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

/// A set of inference modules for constraint propagation.
#[derive(Clone)]
pub(in crate::solver) struct Reasoners {
    sat: SatSolver,
    theories: Vec<TheorySolver>,
    /// Associates each reasoner's ID with its index in the theories vector.
    identities: [u8; 255],
}
impl Reasoners {
    pub fn new(sat: SatSolver, sat_id: WriterId) -> Self {
        let mut reas = Reasoners {
            sat,
            theories: Vec::new(),
            identities: [255u8; 255],
        };
        reas.identities[sat_id.0 as usize] = 0;
        reas
    }

    pub fn add_theory(&mut self, th: TheorySolver) {
        self.identities[th.theory.identity().0 as usize] = (self.theories.len() as u8) + 1;
        self.theories.push(th);
    }
}
impl Explainer for Reasoners {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation) {
        let internal_id = self.identities[cause.writer.0 as usize];
        if internal_id == 0 {
            self.sat.explain(literal, cause.payload, model, explanation);
        } else {
            let theory_id = (internal_id - 1) as usize;
            self.theories[theory_id]
                .theory
                .explain(literal, cause.payload, model, explanation);
        }
    }
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
    constraints: Constraints<Lbl>,
    pub brancher: Box<dyn SearchControl<Lbl> + Send>,
    reasoners: Reasoners,
    decision_level: DecLvl,
    pub stats: Stats,
    /// A data structure with the various communication channels
    /// need to receive/sent updates and commands.
    sync: Synchro,
    /// A queue of literals that we know to be tautologies but that have not been propagated yet.
    /// Invariant: if the queue is non-empty, we are at root level.
    pending_tautologies: Vec<Lit>,
}
impl<Lbl: Label> Solver<Lbl> {
    pub fn new(mut model: Model<Lbl>) -> Solver<Lbl> {
        let sat_id = model.shape.new_write_token();
        let sat = SatSolver::new(sat_id);

        Solver {
            model,
            constraints: Constraints::default(),
            brancher: default_brancher(),
            reasoners: Reasoners::new(sat, sat_id),
            decision_level: DecLvl::ROOT,
            stats: Default::default(),
            sync: Synchro::new(),
            pending_tautologies: vec![],
        }
    }

    pub fn set_brancher(&mut self, brancher: impl SearchControl<Lbl> + 'static + Send) {
        self.brancher = Box::new(brancher)
    }

    pub fn add_theory<T: Theory>(&mut self, init_theory: impl FnOnce(WriterId) -> T) {
        let token = self.model.shape.new_write_token();
        self._add_theory(Box::new(init_theory(token)))
    }

    fn _add_theory(&mut self, theory: Box<dyn Theory>) {
        let module = TheorySolver::new(theory);
        self.reasoners.add_theory(module);
        self.stats.per_module_propagation_time.push(CycleCount::zero());
        self.stats.per_module_conflicts.push(0);
        self.stats.per_module_propagation_loops.push(0);
    }

    pub fn input_stream(&self) -> InputStream {
        self.sync.input_stream()
    }

    pub fn set_solver_output(&mut self, output: Sender<SolverOutput>) {
        self.sync.set_output(output);
    }

    fn set_tautology(&mut self, lit: Lit) {
        debug_assert_eq!(self.model.current_decision_level(), DecLvl::ROOT);
        self.pending_tautologies.push(lit);
    }

    pub fn enforce<Expr: Normalize<T>, T: ReifiableExpr>(&mut self, bool_expr: Expr) {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        self.model.enforce(bool_expr);
        self.post_constraints();
    }
    pub fn enforce_all<Expr: Normalize<T>, T: ReifiableExpr>(&mut self, bools: impl IntoIterator<Item = Expr>) {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        self.model.enforce_all(bools);
        self.post_constraints();
    }

    // TODO: we should clean the call places: it should be invoked as early as possible but after all reasoners are added
    pub fn post_constraints(&mut self) {
        self.constraints.decompose_all(&mut self.model);

        use BindingResult::*;
        let start_time = Instant::now();
        let start_cycles = StartCycleCount::now();

        while let Some((llit, expr)) = self.constraints.pop_next_constraint() {
            let llit = *llit;
            assert_eq!(self.model.current_decision_level(), DecLvl::ROOT);
            match expr {
                &BindTarget::Literal(rlit) => {
                    if self.model.entails(llit) {
                        self.set_tautology(rlit);
                    } else if self.model.entails(!llit) {
                        self.set_tautology(!rlit);
                    } else if self.model.entails(rlit) {
                        self.set_tautology(llit);
                    } else if self.model.entails(!rlit) {
                        self.set_tautology(!llit);
                    } else {
                        // llit => rlit
                        self.reasoners.sat.add_clause([!llit, rlit]);
                        // rlit => llit
                        self.reasoners.sat.add_clause([!rlit, llit]);
                    }
                }
                BindTarget::Expr(expr) => {
                    let expr = expr.clone();
                    // while let Some(binding) = reader.pop(&queue).copied() {
                    let mut supported = false;

                    // expr <=> lit_of_expr
                    match self.reasoners.sat.bind(llit, expr.as_ref(), &mut self.model.state) {
                        Enforced => supported = true,
                        Unsupported => {}
                    }
                    for theory in &mut self.reasoners.theories {
                        match theory.bind(llit, expr.as_ref(), &mut self.model.state) {
                            Enforced => supported = true,
                            Unsupported => {}
                        }
                    }
                    if !supported {
                        panic!("Unsupported binding: {:?} <=> {:?}", llit, expr);
                    }
                }
            }
        }
        self.stats.init_time += start_time.elapsed();
        self.stats.init_cycles += start_cycles.elapsed();
    }

    /// Searches for the first satisfying assignment, returning none if the search
    /// space was exhausted without encountering a solution.
    pub fn solve(&mut self) -> Result<Option<Arc<SavedAssignment>>, Exit> {
        self.post_constraints();
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
                    self.stats.num_restarts += 1;
                }
                None => {
                    log_dec!("=> SOLUTION");
                    // SAT: consistent + no choices left
                    self.stats.solve_time += start_time.elapsed();
                    self.stats.solve_cycles += start_cycles.elapsed();
                    self.stats.num_solutions += 1;
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
        self.post_constraints();
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

                // save the best solution
                best = Some((objective_value, sol));

                // restart at root with a constraint enforcing future solution to improve the objective
                self.stats.num_restarts += 1;
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
        log_dec!("decision: {}", self.model.fmt(decision));
        self.save_state();
        let res = self.model.state.decide(decision);
        assert_eq!(res, Ok(true), "Decision did not result in a valid modification.");
        self.stats.num_decisions += 1;
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
    fn add_conflicting_clause_and_backtrack(&mut self, expl: Disjunction) -> bool {
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
            // make sure brancher has knowledge of all variables.
            self.brancher.import_vars(&self.model);

            // inform the brancher that we are in a conflict state
            self.brancher.conflict(&expl, &self.model);

            // backtrack
            self.restore(dl);
            debug_assert_eq!(self.model.state.value_of_clause(&expl), None);

            if let Some(asserted) = asserted {
                // add clause to sat solver, making sure the asserted literal is set to true
                self.reasoners.sat.add_learnt_clause(expl, asserted);
                self.brancher.asserted_after_conflict(asserted, &self.model)
            } else {
                // no asserted literal, just add a forgettable clause
                self.reasoners.sat.add_forgettable_clause(expl)
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
                        "=> CONFLICT  (size: {}) --  [{}]",
                        conflict.len(),
                        conflict.literals().iter().map(|l| self.model.fmt(*l)).format(", ")
                    );
                    self.sync.notify_learnt(&conflict);
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
    pub fn propagate(&mut self) -> Result<(), Disjunction> {
        self.post_constraints();
        let global_start = StartCycleCount::now();
        while let Some(lit) = self.pending_tautologies.pop() {
            debug_assert_eq!(self.current_decision_level(), DecLvl::ROOT);
            match self.model.state.set(lit, Cause::Decision) {
                Ok(_) => {}
                Err(_) => return Err(Disjunction::new(Vec::new())),
            }
        }

        // we might need to do several rounds of propagation to make sur the first inference engines,
        // can react to the deductions of the latest engines.
        loop {
            let num_events_at_start = self.model.state.num_events();
            let sat_start = StartCycleCount::now();
            self.stats.per_module_propagation_loops[0] += 1;

            // propagate sat engine
            match self.reasoners.sat.propagate(&mut self.model.state) {
                Ok(()) => (),
                Err(explanation) => {
                    // conflict, learnt clause and exit
                    let clause = self.model.state.refine_explanation(explanation, &mut self.reasoners);
                    self.stats.num_conflicts += 1;
                    self.stats.per_module_conflicts[0] += 1;

                    // skip theory propagations to repeat sat propagation,
                    self.stats.propagation_time += global_start.elapsed();
                    self.stats.per_module_propagation_time[0] += sat_start.elapsed();
                    return Err(clause);
                }
            }
            self.stats.per_module_propagation_time[0] += sat_start.elapsed();

            // propagate all theories
            for i in 0..self.reasoners.theories.len() {
                let theory_propagation_start = StartCycleCount::now();
                self.stats.per_module_propagation_loops[i + 1] += 1;
                let th = &mut self.reasoners.theories[i];

                match th.process(&mut self.model.state) {
                    Ok(()) => (),
                    Err(contradiction) => {
                        // contradiction, learn clause and exit
                        let clause = match contradiction {
                            Contradiction::InvalidUpdate(fail) => {
                                self.model.state.clause_for_invalid_update(fail, &mut self.reasoners)
                            }
                            Contradiction::Explanation(expl) => {
                                self.model.state.refine_explanation(expl, &mut self.reasoners)
                            }
                        };
                        self.stats.num_conflicts += 1;
                        self.stats.per_module_conflicts[i + 1] += 1;
                        self.stats.propagation_time += global_start.elapsed();
                        self.stats.per_module_propagation_time[i + 1] += theory_propagation_start.elapsed();
                        return Err(clause);
                    }
                }
                self.stats.per_module_propagation_time[i + 1] += theory_propagation_start.elapsed();
            }

            // we need to do another loop to make sure all reasoners have handled all events if
            //  - new events have been added to the model, and
            //  - we have more than one reasoner (including the sat one). True if we have at least one theory
            let propagate_again =
                num_events_at_start < self.model.state.num_events() && !self.reasoners.theories.is_empty();
            if !propagate_again {
                break;
            }
        }
        self.stats.propagation_time += global_start.elapsed();
        Ok(())
    }

    pub fn print_stats(&self) {
        println!("{}", self.stats);
        for (i, th) in self.reasoners.theories.iter().enumerate() {
            println!("====== Theory({})", i + 1);
            th.print_stats();
        }
    }
}

impl<Lbl> Backtrack for Solver<Lbl> {
    fn save_state(&mut self) -> DecLvl {
        self.decision_level += 1;
        let n = self.decision_level;
        assert_eq!(self.model.save_state(), n);
        assert_eq!(self.brancher.save_state(), n);
        assert_eq!(self.reasoners.sat.save_state(), n);
        for th in &mut self.reasoners.theories {
            assert_eq!(th.save_state(), n);
        }
        n
    }

    fn num_saved(&self) -> u32 {
        debug_assert!({
            let n = self.decision_level.to_int();
            assert_eq!(self.model.num_saved(), n);
            assert_eq!(self.brancher.num_saved(), n);
            assert_eq!(self.reasoners.sat.num_saved(), n);
            for th in &self.reasoners.theories {
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
        self.reasoners.sat.restore(saved_id);
        for th in &mut self.reasoners.theories {
            th.restore(saved_id);
        }
        debug_assert_eq!(self.current_decision_level(), saved_id);
    }
}

impl<Lbl: Label> Clone for Solver<Lbl> {
    fn clone(&self) -> Self {
        Solver {
            model: self.model.clone(),
            constraints: self.constraints.clone(),
            brancher: self.brancher.clone_to_box(),
            reasoners: self.reasoners.clone(),
            decision_level: self.decision_level,
            stats: self.stats.clone(),
            sync: self.sync.clone(),
            pending_tautologies: self.pending_tautologies.clone(),
        }
    }
}

impl<Lbl: Label> Shaped<Lbl> for Solver<Lbl> {
    fn get_shape(&self) -> &ModelShape<Lbl> {
        self.model.get_shape()
    }
}

pub enum BindingResult {
    Enforced,
    Unsupported,
}
