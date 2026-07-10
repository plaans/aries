use crate::prelude::*;

use crate::core::literals::Lits;
use crate::lang::{BoolExpr, IAtom, Store};

/// The `alternative` constraint, that imposes that exactly one of the `alternative` element will be selected to decide the `main` value.
///
/// The constraint defines a `main` elements and a set of alternative elements.
///
/// If the `main` element is present, then exactly one of the alternative element must be present.
/// Furthermore, this selected element must have the same value as the `main` element.
///
/// This original constraint is traditionally state on intervals, where as here it is stated on normal variables.
///
/// See: [`crate::lang::expr::alternative`] for instantiation.
///
/// TODO:
///  - the constraint could be generalized to other variable types (Var, LinTerm, ...)
///  - a version specialized for intervals may be useful (it is a common expectation in scheduling solver, but it would
///    require introducing built-in interval variable types which at this point is not considered)
#[derive(Clone)]
pub struct Alternative {
    main: IAtom,
    alternatives: Vec<IAtom>,
}

impl Alternative {
    pub(crate) fn new(main: IAtom, alternatives: Vec<IAtom>) -> Self {
        Self { main, alternatives }
    }
}

impl<Ctx: Store> BoolExpr<Ctx> for Alternative {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        let a = self;
        let enabler = implicant;

        assert_eq!(ctx.presence(a.main), ctx.presence(enabler));

        // presence of all alternatives
        let presences: Lits = (a.alternatives.iter().map(|alt| ctx.presence(alt.var))).collect();

        // at most one must be present
        for (i, p1) in presences.iter().enumerate() {
            for &p2 in &presences[i + 1..] {
                or([!p1, !p2]).enforce_if(implicant, ctx);
            }
        }
        // at least one alternative must be present
        or(presences).enforce_if(implicant, ctx);

        for &alt in &a.alternatives {
            // lhs = alt (when alt is present)
            eq(alt, self.main).opt_enforce_if(implicant, ctx);
        }

        // redundant constraints that provide stronger propagation by tightening the upper and lower bounds
        eq_max(self.main, self.alternatives.iter().copied()).enforce_if(implicant, ctx);
        eq_min(self.main, self.alternatives.iter().copied()).enforce_if(implicant, ctx);
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        [ctx.presence(self.main)].into()
    }
}
