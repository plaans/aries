use crate::{
    lang::{BoolExpr, ModelView},
    prelude::*,
};

/// Requires that all integer expressions have a different value, ignoring those that are absent.
///
/// This is a generalization of the classical `all_different` constraint
/// where terms that contain an absent variable are ignored.
/// If all variables are mandatory, behaves as the usual `all_different` constraint.
///
/// Scope: always defined (absent variables are ignored).
///
/// The constraint is decomposed into pair-wise difference constraints between all pairs of terms.
pub struct AllDifferent {
    vars: Vec<LinSum>,
}

impl AllDifferent {
    pub fn new(vars: impl IntoIterator<Item = LinSum>) -> Self {
        Self {
            vars: vars.into_iter().collect(),
        }
    }
}

impl<Ctx: ModelView> BoolExpr<Ctx> for AllDifferent {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        for (i, x) in self.vars.iter().enumerate() {
            for y in &self.vars[i + 1..] {
                // If both `x` and `y` are present, they should be different
                neq(x, y).opt_enforce_if(implicant, ctx);
            }
        }
    }

    fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
        Conjunction::tautology()
    }
}
