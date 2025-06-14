pub mod marco;

use std::{collections::BTreeSet, time::Duration};

use aries::core::Lit;

type Mus = BTreeSet<Lit>;
type Mcs = BTreeSet<Lit>;

#[derive(Debug, Clone)]
pub struct MusMcsResult {
    pub muses: Vec<Mus>,
    pub mcses: Vec<Mcs>,
    pub run_time: Option<Duration>,
    pub complete: Option<bool>,
}
