pub mod brancher;
pub mod sat_solver;
pub mod stats;
pub mod theory_solver;

use crate::{Contradiction, Theory};
use aries_backtrack::ObsTrail;
use aries_backtrack::{Backtrack, DecLvl};
use aries_model::lang::{BAtom, BExpr, IAtom, IntCst};
use aries_model::{Model, WriterId};

use crate::solver::brancher::{Brancher, Decision};
use crate::solver::sat_solver::SatSolver;
use crate::solver::stats::Stats;
use crate::solver::theory_solver::TheorySolver;
use aries_model::assignments::{Assignment, SavedAssignment};
use aries_model::int_model::{DiscreteModel, Explainer, Explanation, InferenceCause};

use crate::cpu_time::CycleCount;
use crate::cpu_time::StartCycleCount;
use aries_model::bounds::{Bound, Disjunction};
use env_param::EnvParam;
use std::time::Instant;

pub static OPTIMIZE_USES_LNS: EnvParam<bool> = EnvParam::new("ARIES_SMT_OPTIMIZE_USES_LNS", "true");

struct Reasoners {
    sat: SatSolver,
    theories: Vec<TheorySolver>,
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

pub struct Solver {
    pub model: Model,
    brancher: Brancher,
    reasoners: Reasoners,
    decision_level: DecLvl,
    pub stats: Stats,
}
impl Solver {
    pub fn new(mut model: Model) -> Solver {
        let sat_id = model.new_write_token();
        let sat = SatSolver::new(sat_id);
        Solver {
            model,
            brancher: Brancher::new(),
            reasoners: Reasoners::new(sat, sat_id),
            decision_level: DecLvl::ROOT,
            stats: Default::default(),
        }
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

    pub fn solve(&mut self) -> bool {
        let start_time = Instant::now();
        let start_cycles = StartCycleCount::now();
        loop {
            if !self.propagate_and_backtrack_to_consistent() {
                // UNSAT
                self.stats.solve_time += start_time.elapsed();
                self.stats.solve_cycles += start_cycles.elapsed();
                return false;
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
            if OPTIMIZE_USES_LNS.get() {
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

    /// Integrates a conflicting clause (typically learnt through propagation)
    /// and backtracks to the appropriate level.
    /// As a side effect, the activity of the variables in the clause will be increased.
    /// Returns an error if there is no level at which the clause is not conflicting.
    #[must_use]
    fn add_conflicting_clause_and_backtrack(&mut self, expl: Disjunction) -> bool {
        if let Some(dl) = self.backtrack_level_for_clause(expl.literals()) {
            // backtrack
            self.restore(dl);
            debug_assert_eq!(self.model.discrete.or_value(expl.literals()), None);

            // make sure brancher has knowledge of all variables.
            self.brancher.import_vars(&self.model);

            // bump activity of all variables of the clause
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

    #[must_use]
    pub fn propagate_and_backtrack_to_consistent(&mut self) -> bool {
        let global_start = StartCycleCount::now();
        loop {
            let num_events_at_start = self.model.discrete.num_events();
            let sat_start = StartCycleCount::now();
            self.stats.per_module_propagation_loops[0] += 1;

            match self.reasoners.sat.propagate(&mut self.model.discrete) {
                Ok(()) => (),
                Err(explanation) => {
                    let expl = self.model.discrete.refine_explanation(explanation, &mut self.reasoners);
                    if self.add_conflicting_clause_and_backtrack(expl) {
                        self.stats.num_conflicts += 1;
                        self.stats.per_module_conflicts[0] += 1;

                        // skip theory propagations to repeat sat propagation,
                        self.stats.per_module_propagation_time[0] += sat_start.elapsed();
                        continue;
                    } else {
                        // no level at which the clause is not violated
                        self.stats.propagation_time += global_start.elapsed();
                        self.stats.per_module_propagation_time[0] += sat_start.elapsed();
                        return false; // UNSAT
                    }
                }
            }
            self.stats.per_module_propagation_time[0] += sat_start.elapsed();

            let mut contradiction_found = false;
            for i in 0..self.reasoners.theories.len() {
                let theory_propagation_start = StartCycleCount::now();
                self.stats.per_module_propagation_loops[i + 1] += 1;
                debug_assert!(!contradiction_found);
                let th = &mut self.reasoners.theories[i];

                match th.process(&mut self.model.discrete) {
                    Ok(()) => (),
                    Err(contradiction) => {
                        contradiction_found = true;
                        // contradiction, build the new clause
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
                        self.stats.per_module_propagation_time[i + 1] += theory_propagation_start.elapsed();

                        if self.add_conflicting_clause_and_backtrack(clause) {
                            // skip the rest of the propagations
                            break;
                        } else {
                            return false;
                        }
                    }
                }
                self.stats.per_module_propagation_time[i + 1] += theory_propagation_start.elapsed();
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
        self.stats.propagation_time += global_start.elapsed();
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
