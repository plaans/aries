use crate::backtrack::Backtrack;
use crate::model::bool_model::BoolModel;
use crate::model::lang::*;
use crate::model::{Model, WriterId};
use crate::queues::{QReader, Q};
use crate::solver::{Binding, BindingResult, EnforceResult};
use aries_sat::all::Lit;
use aries_sat::solver::{ConflictHandlingResult, PropagationResult};
use std::convert::TryFrom;
use std::num::NonZeroU32;

type BoolChanges = QReader<(Lit, WriterId)>;

pub struct SatSolver {
    pub(crate) sat: aries_sat::solver::Solver, // TODO: make private
    tautology: Option<Lit>,
    token: WriterId,
    changes: BoolChanges,
}
impl SatSolver {
    pub fn new(token: WriterId, changes: BoolChanges) -> SatSolver {
        SatSolver {
            sat: aries_sat::solver::Solver::default(),
            tautology: None,
            token,
            changes,
        }
    }

    pub fn bind(&mut self, reif: Lit, e: &Expr, bindings: &mut Q<Binding>, model: &mut BoolModel) -> BindingResult {
        match e.fun {
            Fun::And => unimplemented!(),
            Fun::Or => {
                let mut disjuncts = Vec::with_capacity(e.args.len());
                for &a in &e.args {
                    let a = BAtom::try_from(a).expect("not a boolean");
                    let lit = self.reify(a, model);
                    bindings.push(Binding::new(lit, a));
                    disjuncts.push(lit);
                }
                let mut clause = Vec::with_capacity(disjuncts.len() + 1);
                // make reif => disjuncts
                clause.push(!reif);
                disjuncts.iter().for_each(|l| clause.push(*l));
                self.sat.add_clause(&clause);
                for disjunct in disjuncts {
                    // enforce disjunct => reif
                    clause.clear();
                    clause.push(!disjunct);
                    clause.push(reif);
                    self.sat.add_clause(&clause);
                }
                BindingResult::Refined
            }
            _ => BindingResult::Unsupported,
        }
    }

    fn tautology(&mut self) -> Lit {
        if let Some(tauto) = self.tautology {
            tauto
        } else {
            let tauto = self.sat.add_var().true_lit();
            self.tautology = Some(tauto);
            self.sat.add_clause(&[tauto]);
            tauto
        }
    }

    pub fn enforce(&mut self, b: BAtom, i: &mut Model, bindings: &mut Q<Binding>) -> EnforceResult {
        // force literal to be true
        // TODO: we should check if the variable already exists and if not, provide tautology instead
        let lit = self.reify(b, &mut i.bools);
        self.sat.add_clause(&[lit]);

        if let Some(e) = i.expressions.expr_of(b) {
            match e.fun {
                Fun::And => {
                    // TODO: we should enforce all members directly
                    bindings.push(Binding::new(lit, b));
                    EnforceResult::Refined
                }
                Fun::Or => {
                    let mut lits = Vec::with_capacity(e.args.len());
                    for &a in &e.args {
                        let a = BAtom::try_from(a).expect("not a boolean");
                        let lit = self.reify(a, &mut i.bools);
                        bindings.push(Binding::new(lit, a));
                        lits.push(lit);
                    }
                    self.sat.add_clause(&lits);
                    EnforceResult::Refined
                }
                _ => EnforceResult::Reified(self.reify(b, &mut i.bools)),
            }
        } else {
            EnforceResult::Enforced
        }
    }

    fn reify(&mut self, b: BAtom, model: &mut BoolModel) -> Lit {
        let lit = match b.var {
            Some(x) => match model.literal_of(x) {
                Some(lit) => lit,
                None => {
                    let lit = self.sat.add_var().true_lit();
                    model.bind(x, lit);
                    lit
                }
            },
            None => self.tautology(),
        };
        if b.negated {
            !lit
        } else {
            lit
        }
    }

    pub fn propagate(&mut self, model: &mut BoolModel) -> SatPropagationResult {
        // process pending model events
        while let Some((lit, writer)) = self.changes.pop() {
            if writer != self.token {
                self.sat.assume(lit);
            } else {
                debug_assert_eq!(
                    self.sat.get_literal(lit),
                    Some(true),
                    "We set a literal ourselves, but the solver does know aboud id"
                );
            }
        }
        match self.sat.propagate() {
            PropagationResult::Conflict(clause) => {
                // we must handle conflict and backtrack in theory
                match self.sat.handle_conflict(clause) {
                    ConflictHandlingResult::Backtracked {
                        num_backtracks,
                        inferred,
                    } => {
                        model.restore(model.num_saved() - num_backtracks.get());
                        model.set(inferred, self.token);
                        SatPropagationResult::Backtracked(num_backtracks)
                    }
                    ConflictHandlingResult::Unsat => SatPropagationResult::Unsat,
                }
            }
            PropagationResult::Inferred(lits) => {
                if lits.is_empty() {
                    SatPropagationResult::NoOp
                } else {
                    for l in lits {
                        model.set(*l, self.token);
                    }
                    SatPropagationResult::Inferred
                }
            }
        }
    }
}

impl Backtrack for SatSolver {
    fn save_state(&mut self) -> u32 {
        self.sat.save_state().num_decisions() - 1
    }

    fn num_saved(&self) -> u32 {
        self.sat.decision_level().num_decisions()
    }

    fn restore_last(&mut self) {
        if !self.sat.backtrack() {
            panic!("No state to restore.");
        }
    }
}

pub enum SatPropagationResult {
    /// The solver and the inference queue have backtracked `n` (n > 0) to handle a conflict.
    /// A literal (implied by the learnt clause) is placed on the queue.
    /// Note: propagation might not be finished as the learnt literal might
    /// not have been propagated.
    Backtracked(NonZeroU32),
    /// No conflict detected, some inferred literals are placed on the queue
    Inferred,
    /// No inference was made
    NoOp,
    Unsat,
}
