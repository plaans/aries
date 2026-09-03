use crate::core::IntCst;
use crate::core::views::{IntBoundable, Term, VarView};
use crate::lang::{BoolExpr, ModelView};
use crate::prelude::{Disjunction, LinSum, geq, implies};
use crate::reasoners::cp::max::{AtLeastOneGeq, MaxElem};
use itertools::Itertools;
use std::fmt::Debug;

/// Constraint equivalent to `lhs = max { e | e \in rhs }`
///
/// ## Optionality
///
/// In the presence of optional variables, it must be the case that for any `e in rhs`, `prez(e) => prez(lhs)`.
/// This is not enforced by the constraint but will be assumed to hold.
///
/// Furthermore the constraint will enforce that whenever `lhs` is present, then there must be at least
/// one element of `rhs` that is present (otherwise the value of `lhs` would not be defined).
///
/// The scope of this constraint is the scope of the `lhs`
#[derive(Clone)]
pub struct EqMax<Variable> {
    lhs: Variable,
    rhs: Vec<Variable>,
}

impl<Variable> EqMax<Variable> {
    pub(crate) fn new(lhs: Variable, rhs: Vec<Variable>) -> Self {
        Self { lhs, rhs }
    }
}

impl<Ctx: ModelView, Variable> BoolExpr<Ctx> for EqMax<Variable>
where
    Variable: Term + IntBoundable + VarView<Value = IntCst> + Into<LinSum> + Send + Sync + Copy + Debug + 'static,
{
    fn enforce_if(&self, implicant: crate::prelude::Lit, ctx: &mut Ctx) {
        ctx.add_assertion(implies(ctx.presence(self.lhs), ctx.presence(implicant)));

        // at least one alternative must be present
        // prez(lhs) => OR_i  prez(alt_i)
        let mut at_least_one_if_lhs_present = self.rhs.iter().map(|&alt| ctx.presence(alt)).collect_vec();
        at_least_one_if_lhs_present.push(!ctx.presence(self.lhs));
        Disjunction::from_vec(at_least_one_if_lhs_present).opt_enforce_if(implicant, ctx);

        // POST  forall i    lhs >= rhs[i]   (scope: ctx.presence(rhs[i]))
        for item in &self.rhs {
            // self.lhs >= item
            geq(self.lhs, *item).opt_enforce_if(implicant, ctx);

            ctx.add_assertion(implies(ctx.presence(*item), ctx.presence(self.lhs)));
        }

        // POST  OR_i  (ctx.presence(rhs[i])  &&  rhs[i] >= lhs)    [scope: ctx.presence(lhs)]
        let prop = AtLeastOneGeq {
            scope: ctx.presence(implicant),
            active: implicant,
            lhs: self.lhs,
            elements: self
                .rhs
                .iter()
                .map(|&elem| MaxElem::new(elem, ctx.presence(elem)))
                .collect_vec(),
        };
        ctx.enforce_user_propagator(prop);
    }

    fn conj_scope(&self, ctx: &Ctx) -> crate::prelude::Conjunction {
        ctx.presence(self.lhs).into()
    }
}
