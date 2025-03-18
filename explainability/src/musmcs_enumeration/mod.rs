pub mod marco;

use std::{collections::BTreeSet, time::Duration};

use aries::core::Lit;

pub struct MusMcsEnumerationConfig {
    pub return_muses: bool,
    pub return_mcses: bool,
}

#[derive(Debug, Clone)]
pub struct MusMcsEnumerationResult {
    pub muses: Option<Vec<BTreeSet<Lit>>>,
    pub mcses: Option<Vec<BTreeSet<Lit>>>,
    pub run_time: Option<Duration>,
    pub complete: Option<bool>,
}
