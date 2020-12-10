mod atom;
mod boolean;
mod discrete;
mod expr;
mod int;
mod sym;

use std::convert::TryFrom;
use std::hash::Hash;

use aries_collections::create_ref_type;

pub type IntCst = i32;

create_ref_type!(DVar);
create_ref_type!(BVar);

pub use atom::Atom;
pub use boolean::BAtom;
pub use discrete::DAtom;
pub use expr::{Expr, Fun};
pub use int::{IAtom, IVar};
pub use sym::SAtom;

#[derive(Debug)]
pub struct TypeError;

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
        let mut m = Model::default();

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
