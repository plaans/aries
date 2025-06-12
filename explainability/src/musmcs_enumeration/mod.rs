pub mod marco;

use std::{collections::BTreeSet, time::Duration};

use aries::core::Lit;

type Callback = Box<dyn Fn(&BTreeSet<Lit>)>;

pub struct MusMcsEnumerationConfig {
    pub return_muses: bool,
    pub return_mcses: bool,
    pub on_mus_found: Option<Callback>,
    pub on_mcs_found: Option<Callback>,
}

type Mus = BTreeSet<Lit>;
type Mcs = BTreeSet<Lit>;

#[derive(Debug, Clone)]
pub struct MusMcsEnumerationResult {
    pub muses: Option<Vec<Mus>>,
    pub mcses: Option<Vec<Mcs>>,
    pub run_time: Option<Duration>,
    pub complete: Option<bool>,
}
