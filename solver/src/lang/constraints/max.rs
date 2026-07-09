use crate::core::IntCst;
use crate::core::views::{IntBoundable, Term, VarView};
use crate::lang::{BoolExpr, Store};
use crate::prelude::{Disjunction, LinSum, geq};
use crate::reasoners::cp::max::{AtLeastOneGeq, MaxElem};
use itertools::Itertools;
use std::fmt::Debug;

/// Constraint equivalent to `lhs = max { e | e \in rhs }`
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

impl<Ctx: Store, Variable> BoolExpr<Ctx> for EqMax<Variable>
where
    Variable: Term + IntBoundable + VarView<Value = IntCst> + Into<LinSum> + Send + Sync + Copy + Debug + 'static,
{
    fn enforce_if(&self, implicant: crate::prelude::Lit, ctx: &mut Ctx) {
        assert!(ctx.entails(implicant), "Unsupported half reified eqmax constraints.");
        assert_eq!(ctx.presence(self.lhs), ctx.presence(implicant.variable()));

        let scope = ctx.presence(self.lhs);

        // at least one alternative must be present
        // prez(lhs) => OR_i  prez(alt_i)
        let mut at_least_one_if_lhs_present = self.rhs.iter().map(|&alt| ctx.presence(alt)).collect_vec();
        at_least_one_if_lhs_present.push(!ctx.presence(self.lhs));
        Disjunction::from_vec(at_least_one_if_lhs_present).opt_enforce_if(implicant, ctx);

        // POST  forall i    lhs >= rhs[i]   (scope: ctx.presence(rhs[i]))
        for item in &self.rhs {
            // self.lhs >= item.var + item.cst
            geq(self.lhs, *item).opt_enforce_if(implicant, ctx);
            // let item_scope = ctx.presence(item.var);
            // debug_assert!(ctx.implies(item_scope, scope));
            // let alt_value = ctx.tautology_of_scope(item_scope);

            // self.post_constraint(&Constraint::HalfReified(constraint.into(), alt_value))?;
        }

        // POST  OR_i  (ctx.presence(rhs[i])  &&  rhs[i] >= lhs)    [scope: ctx.presence(lhs)]
        let prop = AtLeastOneGeq {
            scope,
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
