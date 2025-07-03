pub mod marco;

use std::collections::BTreeSet;

use crate::{core::Lit, solver::musmcs::marco::Marco};

pub type MusMcsEnumerator<'a, Lbl> = Marco<'a, Lbl>;

/// Type alias for representing a Minimal Unsatisfiable Subset (MUS)
pub type Mus = BTreeSet<Lit>;

/// Type alias for representing a Minimal Correction Set (MCS)
pub type Mcs = BTreeSet<Lit>;

pub enum MusMcs {
    Mus(Mus),
    Mcs(Mcs),
}
