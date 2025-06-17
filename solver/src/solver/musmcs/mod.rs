pub mod marco;

use std::{collections::BTreeSet, time::Duration};

use crate::core::Lit;

pub type Mus = BTreeSet<Lit>;
pub type Mcs = BTreeSet<Lit>;

pub enum MusMcs {
    Mus(Mus),
    Mcs(Mcs),
}

#[derive(Debug, Clone)]
pub struct MusMcsResult {
    pub muses: Vec<Mus>,
    pub mcses: Vec<Mcs>,
    pub run_time: Option<Duration>,
    pub complete: Option<bool>,
}

impl MusMcsResult {
    pub fn iter_muses(&self) -> &[Mus] {
        self.muses.as_slice()
    }
    pub fn iter_mcses(&self) -> &[Mcs] {
        self.muses.as_slice()
    }
    pub fn iter_muses_mcses(&self) -> &[MusMcs] {
        todo!()
    }
}
