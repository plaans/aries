pub mod marco;

use std::collections::BTreeSet;

use aries::core::Lit;

pub struct MusMcsEnumerationConfig {
    pub return_muses: bool,
    pub return_mcses: bool,
}

#[derive(Clone)]
pub struct MusMcsEnumerationResult {
    pub muses: Option<Vec<BTreeSet<Lit>>>,
    pub mcses: Option<Vec<BTreeSet<Lit>>>,
}
