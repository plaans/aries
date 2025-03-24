use std::collections::HashMap;

use aries::core::VarRef;

use crate::aries::Post;

pub trait Encode {
    fn encode(&self, translation: &HashMap<usize, VarRef>) -> Box<dyn Post<usize>>;
}
