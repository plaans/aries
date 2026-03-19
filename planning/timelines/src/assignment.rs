use aries::{core::state::Term, prelude::*};

/// Trait representing the capability to be evaluated (to a givn type) when provided a total assignment.
///
/// TODO: move to `aries` crate and make superseed the provided methods for all atoms.
pub trait Evaluable {
    type Value;

    fn evaluate(&self, value_of_var: impl Fn(VarRef) -> Option<IntCst>) -> Option<Self::Value>;
}

impl Evaluable for VarRef {
    type Value = IntCst;

    fn evaluate(&self, value_of_var: impl Fn(VarRef) -> Option<IntCst>) -> Option<Self::Value> {
        value_of_var(*self)
    }
}
impl Evaluable for SignedVar {
    type Value = IntCst;

    fn evaluate(&self, value_of_var: impl Fn(VarRef) -> Option<IntCst>) -> Option<Self::Value> {
        value_of_var(self.variable()).map(|val| val * self.sign())
    }
}
impl Evaluable for Lit {
    type Value = bool;

    fn evaluate(&self, value_of_var: impl Fn(VarRef) -> Option<IntCst>) -> Option<Self::Value> {
        self.svar().evaluate(value_of_var).map(|val| val <= self.ub_value())
    }
}

impl Evaluable for IVar {
    type Value = IntCst;

    fn evaluate(&self, value_of_var: impl Fn(VarRef) -> Option<IntCst>) -> Option<Self::Value> {
        value_of_var(self.variable())
    }
}
impl Evaluable for IAtom {
    type Value = IntCst;

    fn evaluate(&self, value_of_var: impl Fn(VarRef) -> Option<IntCst>) -> Option<Self::Value> {
        self.var.evaluate(value_of_var).map(|val| val + self.shift)
    }
}

/// Represents a total assignment, i.e., constructing this type is only valid if all variables are bound or absent in the model
///
/// TODO: this type should be removed
pub struct Assignment {
    sol: Solution,
}

impl Assignment {
    pub fn new(doms: &Domains) -> Self {
        Self {
            sol: doms.extract_solution(),
        }
    }
    pub fn shared(sol: Solution) -> Self {
        Self { sol }
    }

    fn value_of_var(&self, var: VarRef) -> Option<IntCst> {
        let doms = &self.sol;
        match doms.present(var) {
            Some(true) => Some(doms.lb(var)),
            Some(false) => None,
            None => panic!("The assignment is not total"),
        }
    }

    pub fn eval<V, E>(&self, expr: E) -> Option<V>
    where
        E: Evaluable<Value = V>,
    {
        expr.evaluate(|v| self.value_of_var(v))
    }
}
