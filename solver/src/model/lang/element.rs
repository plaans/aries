use crate::{
    model::lang::{
        expr::{eq, implies},
        *,
    },
    prelude::*,
};

/// Represent a single option in an element constraint.
#[derive(Copy, Clone, Debug)]
struct ElementOption {
    /// True if the value is selected (provides the value of the expression)
    selector: Lit,
    /// Value given to the expression when ths option is selected.
    value: IAtom,
}

/// An integer expression that is mimick the `element` constraint.
///
/// It is composed of a number of options, each of the form `(selector_i, value_i)`
/// If, for any `i`, `selector_i` is true then the expression is equal to `value_i`.
///
/// Note that the options are not necessarily exclusive: there might more that one selector
/// that are true (which only implies that they have the same value).
#[derive(Default, Clone)]
pub struct Element {
    options: Vec<ElementOption>,
}

impl Element {
    /// Creates a new element expression with no options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a value option and its selector to the element expresion.
    pub fn add_option(&mut self, selector: Lit, value: impl Into<IAtom>) {
        self.options.push(ElementOption {
            selector,
            value: value.into(),
        });
    }
}

impl<Ctx: Store> IntExpr<Ctx> for Element {
    fn enforce_eq_if(&self, implicant: Lit, value: IAtom, ctx: &mut Ctx) {
        let constraint = EqElement {
            variable: value,
            element: self.clone(),
        };
        constraint.opt_enforce_if(implicant, ctx);
    }

    fn bounds(&self, ctx: &Ctx) -> (IntCst, IntCst) {
        let lb = self
            .options
            .iter()
            .map(|e| ctx.lb(e.value))
            .min()
            .unwrap_or(INT_CST_MAX);
        let ub = self
            .options
            .iter()
            .map(|e| ctx.ub(e.value))
            .max()
            .unwrap_or(INT_CST_MIN);
        (lb, ub)
    }
}

/// Element constraint stating that the `variable` must be equal to at least one element for which the selector is true.
///
/// If `variable` is present, then:
///
///   - exists `(sel_i, val_i) in elements` such that `sel_i` is true
///   - forall `(sel_i, val_i) in elements`, `sel_i => (variable == val_i)`
pub struct EqElement {
    variable: IAtom,
    element: Element,
}

impl<Ctx: Store> BoolExpr<Ctx> for EqElement {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        let _span = tracing::debug_span!("EqElement");
        let _span = _span.enter();
        // make sure that the implicant's scope is large enough
        ctx.add_assertion(implies(ctx.presence_literal(l), ctx.presence_literal(self.variable)));

        // at least one esatablisher must hold
        Disjunction::from_iter(self.element.options.iter().map(|a| a.selector)).enforce_if(l, ctx);

        for o in &self.element.options {
            // if `a` is the establishers the the variable must have its value
            implies(o.selector, eq(o.value, self.variable).implicant(ctx)).enforce_if(l, ctx);
            // TODO: we could do a stronger propagation by ensuring that at least each value in the lhs domain is supported by value in the rhs domains

            ctx.add_assertion(implies(o.selector, ctx.presence_literal(self.variable)));
            ctx.add_assertion(implies(o.selector, ctx.presence_literal(o.value)));
        }
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        [ctx.presence_literal(self.variable)].into()
    }
}
