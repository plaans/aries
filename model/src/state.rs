mod cause;
mod domain;
mod domains;
mod event;
mod explanation;
mod int_domains;
mod presence_graph;

use crate::bounds::Lit;
pub use cause::*;
pub use domain::*;
pub use domains::*;
pub use event::*;
pub use explanation::*;
pub use int_domains::*;

/// Represents a triggered event of setting a conflicting literal.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct InvalidUpdate(pub Lit, pub Origin);

/* TODO
#[cfg(test)]
mod tests {
    use crate::bounds::{Lit as ILit, Lit};
    use crate::extensions::Assignment;
    use crate::lang::{BVar, IVar};
    use crate::state::cause::Origin;
    use crate::state::domain::OptDomain;
    use crate::state::explanation::{Explainer, Explanation};
    use crate::state::{Cause, DiscreteModel, InferenceCause, InvalidUpdate, OptDomains};
    use crate::{Model, WriterId};
    use aries_backtrack::Backtrack;
    use std::collections::HashSet;

    #[test]
    fn domain_updates() {
        let mut model = Model::new();
        let a = model.new_ivar(0, 10, "a");

        assert_eq!(model.state.set_lb(a, -1, Cause::Decision), Ok(false));
        assert_eq!(model.state.set_lb(a, 0, Cause::Decision), Ok(false));
        assert_eq!(model.state.set_lb(a, 1, Cause::Decision), Ok(true));
        assert_eq!(model.state.set_ub(a, 11, Cause::Decision), Ok(false));
        assert_eq!(model.state.set_ub(a, 10, Cause::Decision), Ok(false));
        assert_eq!(model.state.set_ub(a, 9, Cause::Decision), Ok(true));
        // domain is [1, 9]
        assert_eq!(model.domain_of(a), (1, 9));

        model.save_state();
        assert_eq!(model.state.set_lb(a, 9, Cause::Decision), Ok(true));
        assert_eq!(
            model.state.set_lb(a, 10, Cause::Decision),
            Err(InvalidUpdate(Lit::geq(a, 10), Origin::DECISION))
        );

        model.restore_last();
        assert_eq!(model.domain_of(a), (1, 9));
        assert_eq!(model.state.set_ub(a, 1, Cause::Decision), Ok(true));
        assert_eq!(
            model.state.set_ub(a, 0, Cause::Decision),
            Err(InvalidUpdate(Lit::leq(a, 0), Origin::DECISION))
        );
    }

    #[test]
    fn test_explanation() {
        let mut model = Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");
        let n = model.new_ivar(0, 10, "n");

        // constraint 0: "a => (n <= 4)"
        // constraint 1: "b => (n >= 5)"

        let writer = WriterId::new(1);

        let cause_a = Cause::inference(writer, 0u32);
        let cause_b = Cause::inference(writer, 1u32);

        #[allow(unused_must_use)]
        let propagate = |model: &mut Model| -> Result<bool, InvalidUpdate> {
            if model.boolean_value_of(a) == Some(true) {
                model.state.set_ub(n, 4, cause_a)?;
            }
            if model.boolean_value_of(b) == Some(true) {
                model.state.set_lb(n, 5, cause_b)?;
            }
            Ok(true)
        };

        struct Expl {
            a: BVar,
            b: BVar,
            n: IVar,
        }
        impl Explainer for Expl {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: ILit,
                _model: &DiscreteModel,
                explanation: &mut Explanation,
            ) {
                assert_eq!(cause.writer, WriterId::new(1));
                match cause.payload {
                    0 => {
                        assert_eq!(literal, ILit::leq(self.n, 4));
                        explanation.push(ILit::is_true(self.a));
                    }
                    1 => {
                        assert_eq!(literal, ILit::geq(self.n, 5));
                        explanation.push(ILit::is_true(self.b));
                    }
                    _ => panic!("unexpected payload"),
                }
            }
        }

        let mut network = Expl { a, b, n };

        propagate(&mut model).unwrap();
        model.save_state();
        model.state.set_lb(a, 1, Cause::Decision).unwrap();
        assert_eq!(model.bounds(a.into()), (1, 1));
        propagate(&mut model).unwrap();
        assert_eq!(model.opt_domain_of(n), OptDomain::Present(0, 4));
        model.save_state();
        model.state.set_lb(n, 1, Cause::Decision).unwrap();
        model.save_state();
        model.state.set_lb(b, 1, Cause::Decision).unwrap();
        let err = match propagate(&mut model) {
            Err(err) => err,
            _ => panic!(),
        };

        let clause = model.state.clause_for_invalid_update(err, &mut network);
        let clause: HashSet<_> = clause.literals().iter().copied().collect();

        // we have three rules
        //  -  !(n <= 4) || !(n >= 5)   (conflict)
        //  -  !a || (n <= 4)           (clause a)
        //  -  !b || (n >= 5)           (clause b)
        // Explanation should perform resolution of the first and last rules for the literal (n >= 5):
        //   !(n <= 4) || !b
        //   !b || (n > 4)      (equivalent to previous)
        let mut expected = HashSet::new();
        expected.insert(ILit::is_false(b));
        expected.insert(ILit::gt(n, 4));
        assert_eq!(clause, expected);
    }

    struct NoExplain;
    impl Explainer for NoExplain {
        fn explain(&mut self, _: InferenceCause, _: Lit, _: &OptDomains, _: &mut Explanation) {
            panic!("No external cause expected")
        }
    }

    #[test]
    fn test_optional_propagation_error() {
        let mut model = Model::new();
        let p = model.new_bvar(0, 1, "p");
        let i = model.new_optional_var(0, 10, p.geq(1), "i");
        let x = model.new_var(0, 10, "x");

        model.save_state();
        assert_eq!(model.set_lb(p, 1, Cause::Decision), Ok(true));
        model.save_state();
        assert_eq!(model.set_ub(i, 5, Cause::Decision), Ok(true));

        // irrelevant event
        model.save_state();
        assert_eq!(model.set_ub(x, 5, Cause::Decision), Ok(true));

        model.save_state();
        assert!(matches!(model.set_lb(i, 6, Cause::Decision), Err(_)));
    }
}
*/
