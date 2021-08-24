pub mod sat_solver;
pub mod search;
pub mod stats;
pub mod theory_solver;

use crate::{Contradiction, Theory};
use aries_backtrack::ObsTrail;
use aries_backtrack::{Backtrack, DecLvl};
use aries_model::lang::{BAtom, BExpr, IAtom, IntCst};
use aries_model::{Model, WriterId};

use crate::solver::sat_solver::SatSolver;
use crate::solver::search::{default_brancher, Decision, SearchControl};
use crate::solver::stats::Stats;
use crate::solver::theory_solver::TheorySolver;
use aries_model::assignments::{Assignment, SavedAssignment};
use aries_model::int_model::{DiscreteModel, Explainer, Explanation, InferenceCause};

use crate::cpu_time::CycleCount;
use crate::cpu_time::StartCycleCount;
use crate::signals::{InputSignal, InputStream, SolverOutput, Synchro};
use aries_model::bounds::{Bound, Disjunction};
use std::fmt::Formatter;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

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
struct Reasoners {
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
    fn explain(&mut self, cause: InferenceCause, literal: Bound, model: &DiscreteModel, explanation: &mut Explanation) {
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

pub struct Solver {
    pub model: Model,
    pub brancher: Box<dyn SearchControl + Send>,
    reasoners: Reasoners,
    decision_level: DecLvl,
    pub stats: Stats,
    /// A data structure with the various communication channels
    /// need to receive/sent updates and commands.
    sync: Synchro,
}
impl Solver {
    pub fn new(mut model: Model) -> Solver {
        let sat_id = model.new_write_token();
        let sat = SatSolver::new(sat_id);

        Solver {
            model,
            brancher: default_brancher(),
            reasoners: Reasoners::new(sat, sat_id),
            decision_level: DecLvl::ROOT,
            stats: Default::default(),
            sync: Synchro::new(),
        }
    }

    pub fn set_brancher(&mut self, brancher: impl SearchControl + 'static + Send) {
        self.brancher = Box::new(brancher)
    }

    pub fn input_stream(&self) -> InputStream {
        self.sync.input_stream()
    }

    pub fn set_solver_output(&mut self, output: Sender<SolverOutput>) {
        self.sync.set_output(output);
    }

    pub fn add_theory(&mut self, theory: Box<dyn Theory>) {
        let module = TheorySolver::new(theory);
        self.reasoners.add_theory(module);
        self.stats.per_module_propagation_time.push(CycleCount::zero());
        self.stats.per_module_conflicts.push(0);
        self.stats.per_module_propagation_loops.push(0);
    }

    /// Impose the constraint that the given boolean atom is true in the final model.
    pub fn enforce(&mut self, constraint: impl Into<BAtom>) {
        self.enforce_all(&[constraint.into()])
    }

    /// Impose the constraint that all given boolean atoms are true in the final model.
    pub fn enforce_all(&mut self, constraints: &[BAtom]) {
        let start_time = Instant::now();
        let start_cycles = StartCycleCount::now();
        let mut queue = ObsTrail::new();
        let mut reader = queue.reader();

        for atom in constraints {
            match self.reasoners.sat.enforce(*atom, &mut self.model, &mut queue) {
                EnforceResult::Enforced => (),
                EnforceResult::Reified(l) => queue.push(Binding::new(l, *atom)),
                EnforceResult::Refined => (),
            }
        }

        while let Some(binding) = reader.pop(&queue).copied() {
            let mut supported = false;

            // if the atom is bound to an expression, get the expression and corresponding literal
            match binding.atom {
                BAtom::Expr(BExpr { expr, negated }) => {
                    let lit_of_expr = if negated { !binding.lit } else { binding.lit };
                    // expr <=> lit_of_expr
                    match self.reasoners.sat.bind(
                        lit_of_expr,
                        self.model.expressions.get(expr),
                        &mut queue,
                        &mut self.model.discrete,
                    ) {
                        BindingResult::Enforced => supported = true,
                        BindingResult::Unsupported => {}
                        BindingResult::Refined => supported = true,
                    }
                    for theory in &mut self.reasoners.theories {
                        match theory.bind(lit_of_expr, expr, &mut self.model, &mut queue) {
                            BindingResult::Enforced => supported = true,
                            BindingResult::Unsupported => {}
                            BindingResult::Refined => supported = true,
                        }
                    }
                }
                BAtom::Cst(true) => {
                    // binding.lit <=> TRUE
                    self.reasoners.sat.add_clause([binding.lit]);
                    supported = true;
                }
                BAtom::Cst(false) => {
                    // binding.lit <=> FALSE
                    self.reasoners.sat.add_clause([!binding.lit]);
                    supported = true;
                }
                BAtom::Bound(l) => {
                    // binding.lit => l
                    self.reasoners.sat.add_clause([!binding.lit, l]);
                    // l => binding.lit
                    self.reasoners.sat.add_clause([!l, binding.lit]);
                    supported = true;
                }
            };

            assert!(supported, "Unsupported binding: {}", self.model.fmt(binding.atom));
        }
        self.stats.init_time += start_time.elapsed();
        self.stats.init_cycles += start_cycles.elapsed();
    }

    /// Searches for the first satisfying assignment, returning none if the search
    /// space was exhausted without encountering a solution.
    pub fn solve(&mut self) -> Result<Option<Arc<SavedAssignment>>, Exit> {
        match self._solve()? {
            SolveResult::AtSolution => Ok(Some(Arc::new(self.model.clone()))),
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
                    // println!("Decision on: {} -- {:?}", self.model.discrete.fmt(lit.variable()), lit);
                    self.decide(lit);
                }
                Some(Decision::Restart) => {
                    self.reset();
                    self.stats.num_restarts += 1;
                }
                None => {
                    // SAT: consistent + no choices left
                    self.stats.solve_time += start_time.elapsed();
                    self.stats.solve_cycles += start_cycles.elapsed();
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
        mut on_new_solution: impl FnMut(IntCst, &SavedAssignment),
    ) -> Result<Option<(IntCst, Arc<SavedAssignment>)>, Exit> {
        let objective = objective.into();
        // best solution found so far
        let mut best = None;
        loop {
            let sol = match self._solve()? {
                SolveResult::AtSolution => {
                    // solver stopped at a solution, this is necessarily an improvement on the best solution found so far
                    let sol = Arc::new(self.model.clone());
                    // notify other solvers that we have found a new solution
                    self.sync.notify_solution_found(sol.clone());
                    let lb = sol.domain_of(objective).0;
                    on_new_solution(lb, &sol);
                    sol
                }
                SolveResult::ExternalSolution(sol) => sol, // a solution was handed out to us by another solver
                SolveResult::Unsat => return Ok(best), // exhausted search space, return the best result found so far
            };

            // determine whether the solution found is an improvement on the previous one (might not be the case if sent by another solver)
            let lb = sol.domain_of(objective).0;
            let is_improvement = match best {
                None => true,
                Some((previous_best, _)) => lb < previous_best,
            };

            if is_improvement {
                // Notify the brancher that a new solution has been found.
                // This enables the use of LNS-like solution and letting the brancher use the values in the best solution
                // as the preferred ones.
                self.brancher.new_assignment_found(lb, sol.clone());

                // save the best solution
                best = Some((lb, sol));

                // restart at root with a constraint enforcing future solution to improve the objective
                self.stats.num_restarts += 1;
                self.reset();
                self.reasoners.sat.add_clause([objective.lt_lit(lb)]);
            }
        }
    }

    pub fn decide(&mut self, decision: Bound) {
        self.save_state();
        // println!("decision: {:?}", decision);
        let res = self.model.discrete.decide(decision);
        assert_eq!(res, Ok(true), "Decision did not result in a valid modification.");
        self.stats.num_decisions += 1;
    }

    /// Determines the appropriate backtrack level for this clause.
    /// Ideally this should be the earliest level at which the clause is unit.
    ///
    /// In the general case, there might not be such level. This means that the two literals
    /// that became violated the latest, are violated at the same decision level.
    /// In this case, we select the latest decision level in which the clause is not violated
    fn backtrack_level_for_clause(&self, clause: &[Bound]) -> Option<DecLvl> {
        debug_assert_eq!(self.model.discrete.or_value(clause), Some(false));
        let mut max = DecLvl::ROOT;
        let mut max_next = DecLvl::ROOT;
        for &lit in clause {
            if let Some(ev) = self.model.discrete.implying_event(!lit) {
                let dl = self.model.discrete.trail().decision_level(ev);
                if dl > max {
                    max_next = max;
                    max = dl;
                } else if dl > max_next {
                    max_next = dl;
                }
            }
        }
        if max == DecLvl::ROOT {
            None
        } else if max == max_next {
            Some(max - 1)
        } else {
            Some(max_next)
        }
    }

    /// Integrates a conflicting clause (typically learnt through conflict analysis)
    /// and backtracks to the appropriate level.
    /// As a side effect, the activity of the variables in the clause will be increased.
    /// Returns `false` if the clause is conflicting at the root and thus constitutes a contradiction.
    #[must_use]
    fn add_conflicting_clause_and_backtrack(&mut self, expl: Disjunction) -> bool {
        // println!("conflict: {:?}", &expl);
        if let Some(dl) = self.backtrack_level_for_clause(expl.literals()) {
            // backtrack
            self.restore(dl);
            debug_assert_eq!(self.model.discrete.or_value(expl.literals()), None);

            // make sure brancher has knowledge of all variables.
            self.brancher.import_vars(&self.model);

            // bump activity of all variables of the clause
            self.brancher.decay_activities();
            for b in expl.literals() {
                self.brancher.bump_activity(b.variable());
            }

            // add clause to sat solver
            self.reasoners.sat.add_forgettable_clause(expl);

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
        let global_start = StartCycleCount::now();

        // we might need to do several rounds of propagation to make sur the first inference engines,
        // can react to the deductions of the latest engines.
        loop {
            let num_events_at_start = self.model.discrete.num_events();
            let sat_start = StartCycleCount::now();
            self.stats.per_module_propagation_loops[0] += 1;

            // propagate sat engine
            match self.reasoners.sat.propagate(&mut self.model.discrete) {
                Ok(()) => (),
                Err(explanation) => {
                    // conflict, learnt clause and exit
                    let clause = self.model.discrete.refine_explanation(explanation, &mut self.reasoners);
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

                match th.process(&mut self.model.discrete) {
                    Ok(()) => (),
                    Err(contradiction) => {
                        // contradiction, learn clause and exit
                        let clause = match contradiction {
                            Contradiction::InvalidUpdate(fail) => {
                                self.model.discrete.clause_for_invalid_update(fail, &mut self.reasoners)
                            }
                            Contradiction::Explanation(expl) => {
                                self.model.discrete.refine_explanation(expl, &mut self.reasoners)
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
                num_events_at_start < self.model.discrete.num_events() && !self.reasoners.theories.is_empty();
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

impl Backtrack for Solver {
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

impl Clone for Solver {
    fn clone(&self) -> Self {
        Solver {
            model: self.model.clone(),
            brancher: self.brancher.clone_to_box(),
            reasoners: self.reasoners.clone(),
            decision_level: self.decision_level,
            stats: self.stats.clone(),
            sync: self.sync.clone(),
        }
    }
}

// TODO: is this needed
#[derive(Copy, Clone, Debug)]
pub struct Binding {
    lit: Bound,
    atom: BAtom,
}
impl Binding {
    pub fn new(lit: Bound, atom: BAtom) -> Binding {
        Binding { lit, atom }
    }
}

pub enum EnforceResult {
    Enforced,
    Reified(Bound),
    Refined,
}

pub enum BindingResult {
    Enforced,
    Unsupported,
    Refined,
}
