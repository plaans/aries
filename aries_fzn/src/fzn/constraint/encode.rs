use std::collections::HashMap;

use aries_solver::core::Var;

use crate::aries::Post;

/// Used to encode a flatzinc constraint into aries constraint.
pub trait Encode {
    /// Return postable aries constraint.
    fn encode(&self, translation: &HashMap<usize, Var>)
    -> Box<dyn Post<usize>>;
}
