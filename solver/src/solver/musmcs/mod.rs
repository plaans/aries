pub mod marco;

use std::collections::BTreeSet;

use crate::{core::Lit, solver::musmcs::marco::Marco};

pub type MusMcsEnumerator<'a, Lbl> = Marco<'a, Lbl>;

/// Type alias for representing a Minimal Unsatisfiable Subset (MUS)
pub type Mus = BTreeSet<Lit>;

/// Type alias for representing a Minimal Correction Set (MCS)
pub type Mcs = BTreeSet<Lit>;

#[derive(Debug, Eq, PartialEq)]
pub enum MusMcs<T> {
    Mus(BTreeSet<T>),
    Mcs(BTreeSet<T>),
}

impl<T> MusMcs<T> {
    /// Project the set to a new representation, ignoring any element with no representation (for which the projection returned `None`).
    ///
    /// This can be used to group elements and ignore others. For instance, the following example ignores negative elements and group others by decade.
    ///
    /// ```rust
    /// use aries::solver::musmcs::MusMcs;
    /// let mus = MusMcs::Mus([-10, 0, 10, 12, 20].into());
    /// let mus2 = mus.project(|&i| if i >= 0 { Some(i/10)} else { None });
    /// assert_eq!(mus2, MusMcs::Mus([0, 1, 2].into()));
    /// ```
    pub fn project<O: Ord>(&self, project_elem: impl Fn(&T) -> Option<O>) -> MusMcs<O> {
        match self {
            MusMcs::Mus(e) => MusMcs::Mus(e.iter().filter_map(project_elem).collect()),
            MusMcs::Mcs(e) => MusMcs::Mcs(e.iter().filter_map(project_elem).collect()),
        }
    }
}
