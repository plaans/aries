use crate::prelude::*;

use crate::core::literals::Lits;
use crate::lang::linear::ScaledVar;
use crate::lang::{BoolExpr, IAtom, Store};
use crate::reasoners::cp::max::{AtLeastOneGeq, MaxElem};
use itertools::Itertools;

/// The `alternative` constraint, that imposes that exactly one of the `alternative` element will be selected to decide the `main` value.
///
/// The constraint defines a `main` elements and a set of alternative elements.
///
/// If the `main` element is present, then exactly one of the alternative element must be present.
/// Furthermore, this selected element must have the same value as the `main` element.
///
/// This original constraint is traditionally state on intervals, where as here it is stated on normal variables.
///
/// See: [`crate::lang::expr::alternative`] for instanciation.
///
/// TODO:
///  - the constraint could be generalized to other variable types (Var, Linterm, ...)
///  - a version specialized for intervals may be useful (it is a common expecteation in scheduling solver, but it would
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
            // debug_assert!(self.model.state.implies(alt_scope, scope));

            // lhs = alt (when alt is present)
            eq(alt, self.main).opt_enforce_if(implicant, ctx);
        }

        // ub(main + cst) <- max_i { ub(var_i) | prez_i }
        // ub(main) <- max_i { ub(var_i) - cst | prez_i }
        ctx.enforce_user_propagator(AtLeastOneGeq {
            scope: ctx.presence(implicant),
            active: implicant,
            lhs: IAtom::from(a.main.var),
            elements: a
                .alternatives
                .iter()
                .map(|alt| MaxElem::new(*alt - a.main.shift, ctx.presence(alt.var)))
                .collect_vec(),
        });

        //  lb(main + cst)  <-   min_i {  lb(var_i) | prez_i }
        //  lb(main)  <-   min_i {  lb(var_i) - cst| prez_i }
        // -ub(-main) <-   min_i { -ub(-var_i) - cst | prez_i }
        // -ub(-main) <- - max_i {  ub(-var_i) + cst | prez_i }
        //  ub(-main) <-   max_i {  ub(-var_i) + cst | prez_i }
        ctx.enforce_user_propagator(AtLeastOneGeq {
            scope: ctx.presence(implicant),
            active: implicant,
            lhs: LinTerm::from(-a.main.var),
            elements: a
                .alternatives
                .iter()
                .map(|alt| {
                    MaxElem::new(
                        LinTerm::new(ScaledVar::new(alt.var, -1), -alt.shift + a.main.shift),
                        ctx.presence(alt.var),
                    )
                })
                .collect_vec(),
        });
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        [ctx.presence(self.main)].into()
    }
}
