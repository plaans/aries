use crate::solver::{Binding, BindingResult, EnforceResult};
use aries_backtrack::Backtrack;
use aries_backtrack::{QReader, Q};
use aries_model::expressions::NExpr;
use aries_model::int_model::DiscreteModel;
use aries_model::lang::*;
use aries_model::{Model, WriterId};
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

    pub fn bind(&mut self, reif: Lit, e: &Expr, bindings: &mut Q<Binding>, model: &mut DiscreteModel) -> BindingResult {
        match e.fun {
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
        let lit = self.reify(b, &mut i.discrete);
        self.sat.add_clause(&[lit]);

        if let BAtom::Expr(b) = b {
            match i.expressions.expr_of(b) {
                NExpr::Pos(e) => match e.fun {
                    Fun::Or => {
                        let mut lits = Vec::with_capacity(e.args.len());
                        for &a in &e.args {
                            let a = BAtom::try_from(a).expect("not a boolean");
                            let lit = self.reify(a, &mut i.discrete);
                            bindings.push(Binding::new(lit, a));
                            lits.push(lit);
                        }
                        self.sat.add_clause(&lits);

                        EnforceResult::Refined
                    }
                    _ => EnforceResult::Reified(lit),
                },
                NExpr::Neg(e) => match e.fun {
                    Fun::Or => {
                        // a negated OR, treat it as and AND
                        for &a in &e.args {
                            let a = BAtom::try_from(a).expect("not a boolean");
                            let lit = self.reify(a, &mut i.discrete);
                            bindings.push(Binding::new(lit, a));
                            self.sat.add_clause(&[!lit]);
                        }

                        EnforceResult::Refined
                    }
                    _ => EnforceResult::Reified(lit),
                },
            }
        } else {
            // Var or constant, enforce at beginning
            EnforceResult::Enforced
        }
    }

    fn reify(&mut self, b: BAtom, model: &mut DiscreteModel) -> Lit {
        match b {
            BAtom::Cst(true) => self.tautology(),
            BAtom::Cst(false) => !self.tautology(),
            BAtom::Var { var, negated } => {
                let lit = model.intern_variable_with(var, || self.sat.add_var().true_lit());
                if negated {
                    !lit
                } else {
                    lit
                }
            }
            BAtom::Expr(e) => {
                let BExpr { expr, negated } = e;
                let lit = model.intern_expr_with(expr, || self.sat.add_var().true_lit());
                if negated {
                    !lit
                } else {
                    lit
                }
            }
        }
    }

    pub fn propagate(
        &mut self,
        model: &mut DiscreteModel,
        on_learnt_clause: impl FnMut(&[Lit]),
    ) -> SatPropagationResult {
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
                match self.sat.handle_conflict(clause, on_learnt_clause) {
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
