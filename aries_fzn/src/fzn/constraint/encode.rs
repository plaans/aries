use std::collections::HashMap;

use aries::core::VarRef;

use crate::aries::Post;

/// Used to encode a flatzinc constraint into aries constraint.
pub trait Encode {
    /// Return postable aries constraint.
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>>;
}
