use aries_solver::core::IntCst;

use super::{TransitionId, Transitions};
use crate::analysis::grounding::ParametersAssignment;
use crate::{EffectOp, IntTerm, SchedEncoder};

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransitionTermsEvaluation {
    // tpe: TransitionType,
    args: smallvec::SmallVec<[IntCst; 4]>,
    val: Option<IntCst>,
    op: EffectOp,
}
impl TransitionTermsEvaluation {
    pub fn new(args: impl Into<smallvec::SmallVec<[IntCst; 4]>>, val: Option<IntCst>, op: EffectOp) -> Self {
        assert!(match op {
            EffectOp::Assign(term) => term.is_cst(),
            EffectOp::Step(_) => todo!(),
        });
        Self {
            /*tpe,*/ args: args.into(),
            val,
            op,
        }
    }
    // pub fn tpe(&self) -> TransitionType {
    //     self.tpe
    // }
    pub fn args_evaluated(&self) -> &[IntCst] {
        &self.args
    }
    pub fn val_evaluated(&self) -> Option<IntCst> {
        self.val
    }
    // pub fn op(&self) -> &EffectOp {
    //     &self.op
    // }
    pub fn op_evaluated(&self) -> Option<IntCst> {
        match &self.op {
            EffectOp::Assign(term) => {
                debug_assert!(term.is_cst());
                Some(term.constant)
            }
            EffectOp::Step(term) => {
                debug_assert!(term.is_cst());
                Some(term.constant)
            } // EffectOp::Erase => None, // IN THE FUTURE ?
        }
    }
    pub fn op_evaluated_as_assign(&self) -> IntCst {
        match &self.op {
            EffectOp::Assign(term) => {
                debug_assert!(term.is_cst());
                term.constant
            }
            _ => panic!("effect operation expected to be an assign"),
        }
    }
}

impl Transitions {
    pub fn evaluate_terms<'a>(
        &'a self,
        trans_id: TransitionId,
        source_grounding: &ParametersAssignment,
        ctx: &'a SchedEncoder,
    ) -> TransitionTermsEvaluation {
        let terms = self.get_terms(trans_id, ctx);

        let args = self.transition_terms_indices_in_source[trans_id]
            .args
            .iter()
            .enumerate()
            .map(|(j, i)| i.map_or(terms.args[j].constant, |i| source_grounding[i.into()]))
            .collect::<smallvec::SmallVec<_>>();

        let val = self.transition_terms_indices_in_source[trans_id]
            .val
            .map(|i| i.map_or(terms.val.unwrap().constant, |i| source_grounding[i.into()]));

        let op = self.transition_terms_indices_in_source[trans_id].op.map_or(
            match terms.op {
                EffectOp::Assign(term) => EffectOp::Assign(IntTerm::int_cst(term.constant)),
                EffectOp::Step(_) => todo!(),
            },
            |i| match terms.op {
                EffectOp::Assign(_) => EffectOp::Assign(IntTerm::int_cst(source_grounding[i.into()])),
                EffectOp::Step(_) => todo!(),
            },
        );

        TransitionTermsEvaluation::new(args, val, op)
    }
    pub fn iter_evaluated_non_constant_terms<'a>(
        &'a self,
        trans_id: TransitionId,
        source_grounding: &ParametersAssignment,
        ctx: &'a SchedEncoder,
    ) -> impl Iterator<Item = (IntTerm, IntCst)> {
        let terms = self.get_terms(trans_id, ctx);

        let val = self.transition_terms_indices_in_source[trans_id].val.map(|i| {
            (
                terms.val.unwrap(),
                i.map_or(terms.val.unwrap().constant, |i| source_grounding[i.into()]),
            )
        });

        let op = self.transition_terms_indices_in_source[trans_id].op.map_or(
            match terms.op {
                EffectOp::Assign(term) => (term, term.constant),
                EffectOp::Step(_) => todo!(),
            },
            |i| match terms.op {
                EffectOp::Assign(term) => (term, source_grounding[i.into()]),
                EffectOp::Step(_) => todo!(),
            },
        );

        self.transition_terms_indices_in_source[trans_id]
            .args
            .iter()
            .enumerate()
            .map(|(j, i)| {
                (
                    terms.args[j],
                    i.map_or(terms.args[j].constant, |i| source_grounding[i.into()]),
                )
            })
            .chain(val)
            .chain([op])
            .filter(|(term, _)| !term.is_cst())
    }
}
