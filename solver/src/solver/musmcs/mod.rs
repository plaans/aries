pub mod marco;

use std::collections::BTreeSet;

use crate::{core::Lit, solver::musmcs::marco::Marco};

pub type MusMcsEnumerator<'a, Lbl> = Marco<'a, Lbl>;

pub type Mus = BTreeSet<Lit>;
pub type Mcs = BTreeSet<Lit>;

pub enum MusMcs {
    Mus(BTreeSet<Lit>),
    Us(BTreeSet<Lit>),
    Mcs(BTreeSet<Lit>),
    Cs(BTreeSet<Lit>),
}
