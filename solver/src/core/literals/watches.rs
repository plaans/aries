use crate::collections::ref_store::RefVec;
use crate::core::*;

/// A set of literals watches on bound changes.
/// The event watches are all on the same bound (i.e. the lower or the upper bound) of a single variable.
#[derive(Clone)]
pub struct WatchSet<Watcher> {
    watches: Vec<Watch<Watcher>>,
}
impl<Watcher> WatchSet<Watcher> {
    pub fn new() -> Self {
        WatchSet { watches: Vec::new() }
    }

    pub fn add_watch(&mut self, watcher: Watcher, literal: Lit) {
        self.watches.push(Watch {
            watcher,
            guard: literal.bound_value(),
        })
    }

    pub fn clear(&mut self) {
        self.watches.clear();
    }

    /// Remove the watch of the given watcher from this set.
    /// The method will panic if there is not exactly one watch for this watcher.
    pub fn remove_watch(&mut self, watcher: Watcher)
    where
        Watcher: Eq,
    {
        let index = self.watches.iter().position(|w| w.watcher == watcher).unwrap();
        self.watches.swap_remove(index);
        debug_assert!(self.watches.iter().all(|w| w.watcher != watcher));
    }

    pub fn is_watched_by(&self, watcher: Watcher, literal: Lit) -> bool
    where
        Watcher: Eq,
    {
        self.watches
            .iter()
            .any(|w| w.watcher == watcher && literal.bound_value().stronger(w.guard))
    }

    pub fn watches_on(&self, literal: Lit) -> impl Iterator<Item = Watcher> + '_
    where
        Watcher: Copy,
    {
        self.watches.iter().filter_map(move |w| {
            if literal.bound_value().stronger(w.guard) {
                Some(w.watcher)
            } else {
                None
            }
        })
    }

    pub fn all_watches(&self) -> impl Iterator<Item = &Watch<Watcher>> + '_ {
        self.watches.iter()
    }

    pub fn move_watches_to(&mut self, literal: Lit, out: &mut WatchSet<Watcher>) {
        let mut i = 0;
        while i < self.watches.len() {
            if literal.bound_value().stronger(self.watches[i].guard) {
                let w = self.watches.swap_remove(i);
                out.watches.push(w);
            } else {
                i += 1
            }
        }
    }
}

impl<Watcher> Default for WatchSet<Watcher> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone)]
pub struct Watch<Watcher> {
    pub watcher: Watcher,
    guard: UpperBound,
}
impl<Watcher> Watch<Watcher> {
    pub fn to_lit(&self, var_bound: SignedVar) -> Lit {
        Lit::from_parts(var_bound, self.guard)
    }
}

/// A datastructure for implementing watches, functionnally equivalent to a `Map<Lit, Set<Watcher>>`
#[derive(Clone)]
pub struct Watches<Watcher> {
    watches: RefVec<SignedVar, WatchSet<Watcher>>,
    empty_watch_set: WatchSet<Watcher>,
}
impl<Watcher> Watches<Watcher> {
    pub fn new() -> Self {
        Watches {
            watches: Default::default(),
            empty_watch_set: WatchSet::new(),
        }
    }
    fn ensure_capacity(&mut self, var: SignedVar) {
        while !self.watches.contains(var) {
            self.watches.push(WatchSet::new());
        }
    }

    pub fn add_watch(&mut self, watcher: Watcher, literal: Lit) {
        self.ensure_capacity(literal.svar());
        self.watches[literal.svar()].add_watch(watcher, literal);
    }

    pub fn is_watched_by(&self, literal: Lit, watcher: Watcher) -> bool
    where
        Watcher: Eq,
    {
        if self.watches.contains(literal.svar()) {
            self.watches[literal.svar()].is_watched_by(watcher, literal)
        } else {
            false
        }
    }

    pub fn remove_watch(&mut self, watcher: Watcher, literal: Lit)
    where
        Watcher: Eq,
    {
        self.ensure_capacity(literal.svar());
        self.watches[literal.svar()].remove_watch(watcher);
    }

    /// Get the watchers triggered by the literal becoming true
    /// If the literal is (n <= 4), it should trigger watches on (n <= 4), (n <= 5), ...
    /// If the literal is (n > 5), it should trigger watches on (n > 5), (n > 4), (n > 3), ...
    pub fn watches_on(&self, literal: Lit) -> impl Iterator<Item = Watcher> + '_
    where
        Watcher: Copy,
    {
        let set = if self.watches.contains(literal.svar()) {
            &self.watches[literal.svar()]
        } else {
            &self.empty_watch_set
        };
        set.watches_on(literal)
    }

    pub fn move_watches_to(&mut self, literal: Lit, out: &mut WatchSet<Watcher>) {
        if self.watches.contains(literal.svar()) {
            self.watches[literal.svar()].move_watches_to(literal, out)
        }
    }
}

impl<Watcher> Default for Watches<Watcher> {
    fn default() -> Self {
        Watches::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Lit;

    #[test]
    fn test_watches() {
        let a = VarRef::from_u32(1);
        let b = VarRef::from_u32(2);

        let watches = &mut Watches::new();

        watches.add_watch(1, Lit::leq(a, 1));
        watches.add_watch(2, Lit::leq(a, 2));
        watches.add_watch(3, Lit::leq(a, 3));

        watches.add_watch(1, Lit::geq(a, 1));
        watches.add_watch(2, Lit::geq(a, 2));
        watches.add_watch(3, Lit::geq(a, 3));

        let check_watches_on = |watches: &Watches<_>, bound, mut expected: Vec<_>| {
            let mut res: Vec<_> = watches.watches_on(bound).collect();
            res.sort_unstable();
            expected.sort_unstable();
            assert_eq!(res, expected);
        };
        check_watches_on(watches, Lit::leq(a, 0), vec![1, 2, 3]);
        check_watches_on(watches, Lit::leq(a, 1), vec![1, 2, 3]);
        check_watches_on(watches, Lit::leq(a, 2), vec![2, 3]);
        check_watches_on(watches, Lit::leq(a, 3), vec![3]);
        check_watches_on(watches, Lit::leq(a, 4), vec![]);

        check_watches_on(watches, Lit::geq(a, 0), vec![]);
        check_watches_on(watches, Lit::geq(a, 1), vec![1]);
        check_watches_on(watches, Lit::geq(a, 2), vec![1, 2]);
        check_watches_on(watches, Lit::geq(a, 3), vec![1, 2, 3]);
        check_watches_on(watches, Lit::geq(a, 4), vec![1, 2, 3]);

        watches.remove_watch(2, Lit::leq(a, 2));
        watches.remove_watch(3, Lit::geq(a, 3));
        check_watches_on(watches, Lit::leq(a, 0), vec![1, 3]);
        check_watches_on(watches, Lit::leq(a, 1), vec![1, 3]);
        check_watches_on(watches, Lit::leq(a, 2), vec![3]);
        check_watches_on(watches, Lit::leq(a, 3), vec![3]);
        check_watches_on(watches, Lit::leq(a, 4), vec![]);

        check_watches_on(watches, Lit::geq(a, 0), vec![]);
        check_watches_on(watches, Lit::geq(a, 1), vec![1]);
        check_watches_on(watches, Lit::geq(a, 2), vec![1, 2]);
        check_watches_on(watches, Lit::geq(a, 3), vec![1, 2]);
        check_watches_on(watches, Lit::geq(a, 4), vec![1, 2]);

        watches.add_watch(2, Lit::leq(a, 2));
        watches.add_watch(3, Lit::geq(a, 3));
        check_watches_on(watches, Lit::leq(a, 0), vec![1, 2, 3]);
        check_watches_on(watches, Lit::leq(a, 1), vec![1, 2, 3]);
        check_watches_on(watches, Lit::leq(a, 2), vec![2, 3]);
        check_watches_on(watches, Lit::leq(a, 3), vec![3]);
        check_watches_on(watches, Lit::leq(a, 4), vec![]);

        check_watches_on(watches, Lit::geq(a, 0), vec![]);
        check_watches_on(watches, Lit::geq(a, 1), vec![1]);
        check_watches_on(watches, Lit::geq(a, 2), vec![1, 2]);
        check_watches_on(watches, Lit::geq(a, 3), vec![1, 2, 3]);
        check_watches_on(watches, Lit::geq(a, 4), vec![1, 2, 3]);

        // no watches on a different variable
        check_watches_on(watches, Lit::leq(b, 0), vec![]);
        check_watches_on(watches, Lit::leq(b, 1), vec![]);
        check_watches_on(watches, Lit::leq(b, 2), vec![]);
        check_watches_on(watches, Lit::leq(b, 3), vec![]);
        check_watches_on(watches, Lit::leq(b, 4), vec![]);

        check_watches_on(watches, Lit::geq(b, 0), vec![]);
        check_watches_on(watches, Lit::geq(b, 1), vec![]);
        check_watches_on(watches, Lit::geq(b, 2), vec![]);
        check_watches_on(watches, Lit::geq(b, 3), vec![]);
        check_watches_on(watches, Lit::geq(b, 4), vec![]);
    }
}
