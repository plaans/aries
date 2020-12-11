mod atom;
mod boolean;
mod discrete;
mod expr;
mod int;
mod sym;
mod variables;

use std::convert::TryFrom;
use std::hash::Hash;

use aries_collections::create_ref_type;

pub type IntCst = i32;

create_ref_type!(DVar);

pub use atom::Atom;
pub use boolean::{BAtom, BVar};
pub use discrete::{DAtom, DiscreteType};
pub use expr::{Expr, Fun};
pub use int::{IAtom, IVar};

pub use sym::{SAtom, SVar, VarOrSym};
pub use variables::Variable;

#[derive(Debug)]
pub enum ConversionError {
    TypeError,
    NotConstant,
    NotVariable,
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionError::TypeError => write!(f, "type error"),
            ConversionError::NotConstant => write!(f, "not a constant"),
            ConversionError::NotVariable => write!(f, "not a variable"),
        }
    }
}

impl std::error::Error for ConversionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Model;

    fn check(m: &Model, x: impl Into<Atom>, result: &str) {
        assert_eq!(m.fmt(x).to_string(), result);
    }

    #[test]
    #[ignore] // TODO: fix syntax printing
    fn test_syntax() {
        let mut m = Model::new();

        let a = m.new_ivar(0, 10, "a");
        check(&m, a, "a");

        let b = m.new_ivar(0, 10, "b");

        let x = b + 1;
        check(&m, x, "(+ b 1)");

        let x = b - 1;
        check(&m, x, "(- b 1)");

        let x = x + 1;
        check(&m, x, "b");

        let x = m.leq(a + 1, 6);
        check(&m, x, "(<= (+ a 1) 6)");

        let x = m.eq(a - 3, b);
        check(&m, x, "(= (- a 3) b)");

        let x = m.implies(true, x);
        check(&m, x, "(or false (= (- a 3) b))")
    }
}
