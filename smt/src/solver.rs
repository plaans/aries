pub mod brancher;
pub mod sat_solver;
pub mod stats;
pub mod theory_solver;

use crate::{Contradiction, Theory};
use aries_backtrack::Backtrack;
use aries_backtrack::ObsTrail;
use aries_model::lang::{BAtom, BExpr, Disjunction, IAtom, IntCst};
use aries_model::{Model, WriterId};

use crate::solver::brancher::{Brancher, Decision};
use crate::solver::sat_solver::SatSolver;
use crate::solver::stats::Stats;
use crate::solver::theory_solver::TheorySolver;
use aries_model::assignments::{Assignment, SavedAssignment};
use aries_model::int_model::{DiscreteModel, Explainer, Explanation, InferenceCause};
use aries_model::lang::Bound;
use env_param::EnvParam;
use std::time::Instant;

pub static OPTIMIZE_USES_LNS: EnvParam<bool> = EnvParam::new("ARIES_SMT_OPTIMIZE_USES_LNS", "true");

struct Reasoners {
    sat: SatSolver,
    theories: Vec<TheorySolver>,
}
impl Explainer for Reasoners {
    fn explain(&mut self, cause: InferenceCause, literal: Bound, model: &DiscreteModel, explanation: &mut Explanation) {
        if cause.writer == SMTSolver::sat_token() {
            self.sat.explain(literal, cause.payload, model, explanation);
        } else {
            let theory_id = (cause.writer.0 - 2) as usize;
            self.theories[theory_id]
                .theory
                .explain(literal, cause.payload, model, explanation);
        }
    }
}

pub struct SMTSolver {
    pub model: Model,
    brancher: Brancher,
    reasoners: Reasoners,
    num_saved_states: u32,
    pub stats: Stats,
}
impl SMTSolver {
    fn sat_token() -> WriterId {
        WriterId::new(1)
    }

    #[allow(dead_code)]
    fn theory_token(theory_num: u8) -> WriterId {
        WriterId::new(2 + theory_num)
    }

    pub fn new(mut model: Model) -> SMTSolver {
        let sat = SatSolver::new(Self::sat_token(), &mut model);
        SMTSolver {
            model,
            brancher: Brancher::new(),
            reasoners: Reasoners {
                sat,
                theories: Vec::new(),
            },
            num_saved_states: 0,
            stats: Default::default(),
        }
    }
    pub fn add_theory(&mut self, theory: Box<dyn Theory>) {
        let module = TheorySolver::new(theory);
        self.reasoners.theories.push(module);
        self.stats.per_module_propagation_time.push(0.0);
        self.stats.per_module_conflicts.push(0);
        self.stats.per_module_propagation_loops.push(0);
    }

    /// Impose the constraint that the given boolean atom is true in the final model.
    pub fn enforce(&mut self, constraint: impl Into<BAtom>) {
        self.enforce_all(&[constraint.into()])
    }

    /// Impose the constraint that all given boolean atoms are true in the final model.
    pub fn enforce_all(&mut self, constraints: &[BAtom]) {
        let start = Instant::now();
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
            let var = binding.lit.variable();
            if !self.brancher.is_declared(var) {
                self.brancher.declare(var);
                self.brancher.enqueue(var);
            }
            let mut supported = false;

            // if the atom is bound to an expression, get the expression and corresponding literal
            let expr = match binding.atom {
                BAtom::Expr(BExpr { expr, negated }) => {
                    let lit_of_expr = if negated { !binding.lit } else { binding.lit };
                    Some((expr, lit_of_expr))
                }
                _ => None,
            };
            // if the BAtom has not a corresponding expr, then it is a free variable and we can stop.

            if let Some((expr, lit)) = expr {
                match self.reasoners.sat.bind(
                    lit,
                    self.model.expressions.get(expr),
                    &mut queue,
                    &mut self.model.discrete,
                ) {
                    BindingResult::Enforced => supported = true,
                    BindingResult::Unsupported => {}
                    BindingResult::Refined => supported = true,
                }
                for theory in &mut self.reasoners.theories {
                    match theory.bind(lit, expr, &mut self.model, &mut queue) {
                        BindingResult::Enforced => supported = true,
                        BindingResult::Unsupported => {}
                        BindingResult::Refined => supported = true,
                    }
                }
            } else {
                // standalone boolean variable or constant, it was enforced by the call to sat.enforce
                supported = true;
            }

            assert!(supported, "Unsupported binding: {}", self.model.fmt(binding.atom));
        }
        self.stats.init_time += start.elapsed().as_secs_f64()
    }

    pub fn solve(&mut self) -> bool {
        let start = Instant::now();
        loop {
            if !self.propagate_and_backtrack_to_consistent() {
                // UNSAT
                self.stats.solve_time += start.elapsed().as_secs_f64();
                return false;
            }
            match self.brancher.next_decision(&self.stats, &self.model) {
                Some(Decision::SetLiteral(lit)) => {
                    self.decide(lit);
                }
                Some(Decision::Restart) => {
                    self.reset();
                    self.stats.num_restarts += 1;
                }
                None => {
                    // SAT: consistent + no choices left
                    self.stats.solve_time += start.elapsed().as_secs_f64();
                    return true;
                }
            }
        }
    }

    pub fn minimize(&mut self, objective: impl Into<IAtom>) -> Option<(IntCst, SavedAssignment)> {
        self.minimize_with(objective, |_, _| ())
    }

    pub fn minimize_with(
        &mut self,
        objective: impl Into<IAtom>,
        mut on_new_solution: impl FnMut(IntCst, &SavedAssignment),
    ) -> Option<(IntCst, SavedAssignment)> {
        let objective = objective.into();
        let mut result = None;
        while self.solve() {
            let lb = self.model.domain_of(objective).0;

            let sol = SavedAssignment::from_model(&self.model);
            if *OPTIMIZE_USES_LNS.get() {
                // LNS requested, set the default values of all variables to the one of
                // the best solution. As a result, the solver will explore the solution space
                // around the incumbent solution, only pushed away by the learnt clauses.
                self.brancher.set_default_values_from(&self.model);
            }
            on_new_solution(lb, &sol);
            result = Some((lb, sol));
            self.stats.num_restarts += 1;
            self.reset();
            let improved = self.model.lt(objective, lb);
            self.enforce_all(&[improved]);
        }
        result
    }

    pub fn decide(&mut self, decision: Bound) {
        self.save_state();
        self.model.discrete.decide(decision).unwrap();
        self.stats.num_decisions += 1;
    }

    /// Determines the appropriate backtrack level for this clause.
    /// Ideally this should be the earliest level at which the clause is unit
    ///
    /// In the general case, there might not be such level. This means that the two literals
    /// that became violated the latest, are violated at the same decision level.
    /// In this case, we backtrack to the latest decision level in which the clause is not violated
    fn backtrack_level_for_clause(&self, clause: &[Bound]) -> Option<usize> {
        debug_assert_eq!(self.model.discrete.or_value(clause), Some(false));
        let mut max = 0usize;
        let mut max_next = 0usize;
        for lit in clause {
            if let Some(ev) = self.model.discrete.implying_event(&!*lit) {
                if ev.decision_level > max {
                    max_next = max;
                    max = ev.decision_level;
                } else if ev.decision_level > max_next {
                    max_next = ev.decision_level;
                }
            }
        }
        if max == 0 {
            None
        } else if max == max_next {
            Some(max - 1)
        } else {
            Some(max_next)
        }
    }

    /// Integrates a conflicting clause (typically learnt through propagation)
    /// and backtracks to the appropriate level.
    /// As a side effect, the activity of the variables in the clause will be increased.
    /// Returns an error if there is no level at which the clause is not conflicting.
    #[must_use]
    fn add_conflicting_clause_and_backtrack(&mut self, expl: Disjunction) -> bool {
        if let Some(dl) = self.backtrack_level_for_clause(expl.literals()) {
            // backtrack
            self.restore(dl as u32);
            debug_assert_eq!(self.model.discrete.or_value(expl.literals()), None);

            // add clause to sat solver
            self.reasoners.sat.add_forgettable_clause(expl.literals());

            // bump activity of all variables of the clause
            for b in expl.literals() {
                self.brancher.bump_activity(b.variable());
            }
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn propagate_and_backtrack_to_consistent(&mut self) -> bool {
        let global_start = Instant::now();
        loop {
            let num_events_at_start = self.model.discrete.num_events();
            let sat_start = Instant::now();
            self.stats.per_module_propagation_loops[0] += 1;

            match self.reasoners.sat.propagate(&mut self.model) {
                Ok(()) => (),
                Err(explanation) => {
                    let expl = self.model.discrete.refine_explanation(explanation, &mut self.reasoners);
                    if self.add_conflicting_clause_and_backtrack(expl) {
                        self.stats.num_conflicts += 1;
                        self.stats.per_module_conflicts[0] += 1;

                        // skip theory propagations to repeat sat propagation,
                        self.stats.per_module_propagation_time[0] += sat_start.elapsed().as_secs_f64();
                    } else {
                        // no level at which the clause is not violated
                        self.stats.propagation_time += global_start.elapsed().as_secs_f64();
                        self.stats.per_module_propagation_time[0] += sat_start.elapsed().as_secs_f64();
                        return false; // UNSAT
                    }
                }
            }
            self.stats.per_module_propagation_time[0] += sat_start.elapsed().as_secs_f64();

            let mut contradiction_found = false;
            for i in 0..self.reasoners.theories.len() {
                let theory_propagation_start = Instant::now();
                self.stats.per_module_propagation_loops[i + 1] += 1;
                debug_assert!(!contradiction_found);
                let th = &mut self.reasoners.theories[i];

                match th.process(&mut self.model.writer(Self::theory_token(i as u8))) {
                    Ok(()) => (),
                    Err(contradiction) => {
                        contradiction_found = true;
                        // contradiction, build the new clause
                        let clause = match contradiction {
                            Contradiction::EmptyDomain(var) => {
                                self.model.discrete.explain_empty_domain(var, &mut self.reasoners)
                            }
                            Contradiction::Explanation(expl) => {
                                self.model.discrete.refine_explanation(expl, &mut self.reasoners)
                            }
                        };
                        if self.add_conflicting_clause_and_backtrack(clause) {
                            // skip the rest of the propagations
                            self.stats.per_module_conflicts[i + 1] += 1;
                            self.stats.per_module_propagation_time[i + 1] +=
                                theory_propagation_start.elapsed().as_secs_f64();
                            break;
                        } else {
                            self.stats.per_module_conflicts[i + 1] += 1;
                            self.stats.per_module_propagation_time[i + 1] +=
                                theory_propagation_start.elapsed().as_secs_f64();
                            return false;
                        }
                    }
                }
                self.stats.per_module_propagation_time[i + 1] += theory_propagation_start.elapsed().as_secs_f64();
            }

            #[allow(clippy::needless_bool, clippy::if_same_then_else)]
            let propagate_again = if contradiction_found {
                // a contradiction was found and thus a clause added. We need to propagate again
                true
            } else if num_events_at_start < self.model.discrete.num_events() && !self.reasoners.theories.is_empty() {
                // new events have been added to the model and we have more than one reasoner
                // we need to do another loop to make sure all reasoners have handled all events
                true
            } else {
                false
            };
            if !propagate_again {
                break;
            }
        }
        self.stats.propagation_time += global_start.elapsed().as_secs_f64();
        true
    }

    pub fn print_stats(&self) {
        println!("{}", self.stats);
        for (i, th) in self.reasoners.theories.iter().enumerate() {
            println!("====== Theory({})", i + 1);
            th.print_stats();
        }
    }
}

impl Backtrack for SMTSolver {
    fn save_state(&mut self) -> u32 {
        self.num_saved_states += 1;
        let n = self.num_saved_states - 1;
        assert_eq!(self.model.save_state(), n);
        assert_eq!(self.brancher.save_state(), n);
        assert_eq!(self.reasoners.sat.save_state(), n);
        for th in &mut self.reasoners.theories {
            assert_eq!(th.save_state(), n);
        }
        n
    }

    fn num_saved(&self) -> u32 {
        self.num_saved_states
    }

    fn restore_last(&mut self) {
        assert!(self.num_saved() > 0, "No state to restore");
        let last = self.num_saved() - 1;
        self.restore(last);
        self.num_saved_states -= 1;
    }

    fn restore(&mut self, saved_id: u32) {
        self.num_saved_states = saved_id;
        self.model.restore(saved_id);
        self.brancher.restore(saved_id);
        self.reasoners.sat.restore(saved_id);
        for th in &mut self.reasoners.theories {
            th.restore(saved_id);
        }
    }
}

// TODO: is this needed
#[derive(Copy, Clone)]
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
