use aries_collections::ref_store::RefVec;

use crate::bounds::{Bound, Relation};
use crate::lang::{IntCst, VarRef};
use std::fmt::Debug;

#[derive(Debug)]
pub struct LBWatch<Watcher> {
    pub watcher: Watcher,
    pub guard: IntCst,
}

impl<Watcher> LBWatch<Watcher> {
    pub fn to_lit(&self, var: VarRef) -> Bound {
        Bound::gt(var, self.guard)
    }
}

#[derive(Debug)]
pub struct UBWatch<Watcher> {
    pub watcher: Watcher,
    pub guard: IntCst,
}

impl<Watcher> UBWatch<Watcher> {
    pub fn to_lit(&self, var: VarRef) -> Bound {
        Bound::leq(var, self.guard)
    }
}

pub struct Watches<Watcher> {
    on_lb: RefVec<VarRef, Vec<LBWatch<Watcher>>>,
    on_ub: RefVec<VarRef, Vec<UBWatch<Watcher>>>,
}
impl<Watcher: Copy + Eq> Watches<Watcher> {
    pub fn new() -> Self {
        Watches {
            on_lb: Default::default(),
            on_ub: Default::default(),
        }
    }
    fn ensure_capacity(&mut self, var: VarRef) {
        while !self.on_ub.contains(var) {
            self.on_ub.push(Vec::new());
            self.on_lb.push(Vec::new());
        }
    }

    pub fn add_watch(&mut self, clause: Watcher, literal: Bound) {
        self.ensure_capacity(literal.variable());

        match literal.relation() {
            Relation::LEQ => self.on_ub[literal.variable()].push(UBWatch {
                watcher: clause,
                guard: literal.value(),
            }),
            Relation::GT => self.on_lb[literal.variable()].push(LBWatch {
                watcher: clause,
                guard: literal.value(),
            }),
        }
    }

    pub fn pop_all_lb_watches(&mut self, var: VarRef) -> Vec<LBWatch<Watcher>> {
        self.ensure_capacity(var);
        let mut tmp = Vec::new();
        std::mem::swap(&mut tmp, &mut self.on_lb[var]);
        tmp
    }
    pub fn pop_all_up_watches(&mut self, var: VarRef) -> Vec<UBWatch<Watcher>> {
        self.ensure_capacity(var);
        let mut tmp = Vec::new();
        std::mem::swap(&mut tmp, &mut self.on_ub[var]);
        tmp
    }

    pub fn is_watched_by(&self, literal: Bound, clause: Watcher) -> bool {
        match literal.relation() {
            Relation::LEQ => self.on_ub[literal.variable()]
                .iter()
                .any(|watch| watch.watcher == clause && watch.guard <= literal.value()),

            Relation::GT => self.on_lb[literal.variable()]
                .iter()
                .any(|watch| watch.watcher == clause && watch.guard >= literal.value()),
        }
    }

    pub fn remove_watch(&mut self, clause: Watcher, literal: Bound) {
        let var = literal.variable();
        match literal.relation() {
            Relation::LEQ => {
                let index = self.on_ub[var].iter().position(|w| w.watcher == clause).unwrap();
                self.on_ub[var].swap_remove(index);
                debug_assert!(self.on_ub[var].iter().all(|w| w.watcher != clause));
            }
            Relation::GT => {
                let index = self.on_lb[var].iter().position(|w| w.watcher == clause).unwrap();
                self.on_lb[var].swap_remove(index);
                debug_assert!(self.on_lb[var].iter().all(|w| w.watcher != clause));
            }
        }
    }

    /// Get the watchers triggered by the literal becoming true
    /// If the literal is (n <= 4), it should trigger watches on (n <= 4), (n <= 5), ...
    /// If the literal is (n > 5), it should trigger watches on (n > 5), (n > 4), (n > 3), ...
    pub fn watches_on(&self, literal: Bound) -> Box<dyn Iterator<Item = Watcher> + '_> {
        if !self.on_ub.contains(literal.variable()) {
            return Box::new(std::iter::empty());
        }
        let var = literal.variable();
        let val = literal.value();
        match literal.relation() {
            Relation::LEQ => {
                Box::new(
                    self.on_ub[var]
                        .iter()
                        .filter_map(move |w| if w.guard >= val { Some(w.watcher) } else { None }),
                )
            }
            Relation::GT => {
                Box::new(
                    self.on_lb[var]
                        .iter()
                        .filter_map(move |w| if w.guard <= val { Some(w.watcher) } else { None }),
                )
            }
        }
    }
}

impl<Watcher> Default for Watches<Watcher> {
    fn default() -> Self {
        Watches {
            on_lb: Default::default(),
            on_ub: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounds::Bound;
    use crate::Model;

    #[test]
    fn test_watches() {
        let mut model = Model::new();
        let a = model.new_ivar(0, 10, "a");
        let b = model.new_ivar(0, 10, "b");

        let watches = &mut Watches::new();

        watches.add_watch(1, Bound::leq(a, 1));
        watches.add_watch(2, Bound::leq(a, 2));
        watches.add_watch(3, Bound::leq(a, 3));

        watches.add_watch(1, Bound::geq(a, 1));
        watches.add_watch(2, Bound::geq(a, 2));
        watches.add_watch(3, Bound::geq(a, 3));

        let check_watches_on = |watches: &Watches<_>, bound, mut expected: Vec<_>| {
            let mut res: Vec<_> = watches.watches_on(bound).collect();
            res.sort_unstable();
            expected.sort_unstable();
            assert_eq!(res, expected);
        };
        check_watches_on(watches, Bound::leq(a, 0), vec![1, 2, 3]);
        check_watches_on(watches, Bound::leq(a, 1), vec![1, 2, 3]);
        check_watches_on(watches, Bound::leq(a, 2), vec![2, 3]);
        check_watches_on(watches, Bound::leq(a, 3), vec![3]);
        check_watches_on(watches, Bound::leq(a, 4), vec![]);

        check_watches_on(watches, Bound::geq(a, 0), vec![]);
        check_watches_on(watches, Bound::geq(a, 1), vec![1]);
        check_watches_on(watches, Bound::geq(a, 2), vec![1, 2]);
        check_watches_on(watches, Bound::geq(a, 3), vec![1, 2, 3]);
        check_watches_on(watches, Bound::geq(a, 4), vec![1, 2, 3]);

        watches.remove_watch(2, Bound::leq(a, 2));
        watches.remove_watch(3, Bound::geq(a, 3));
        check_watches_on(watches, Bound::leq(a, 0), vec![1, 3]);
        check_watches_on(watches, Bound::leq(a, 1), vec![1, 3]);
        check_watches_on(watches, Bound::leq(a, 2), vec![3]);
        check_watches_on(watches, Bound::leq(a, 3), vec![3]);
        check_watches_on(watches, Bound::leq(a, 4), vec![]);

        check_watches_on(watches, Bound::geq(a, 0), vec![]);
        check_watches_on(watches, Bound::geq(a, 1), vec![1]);
        check_watches_on(watches, Bound::geq(a, 2), vec![1, 2]);
        check_watches_on(watches, Bound::geq(a, 3), vec![1, 2]);
        check_watches_on(watches, Bound::geq(a, 4), vec![1, 2]);

        watches.add_watch(2, Bound::leq(a, 2));
        watches.add_watch(3, Bound::geq(a, 3));
        check_watches_on(watches, Bound::leq(a, 0), vec![1, 2, 3]);
        check_watches_on(watches, Bound::leq(a, 1), vec![1, 2, 3]);
        check_watches_on(watches, Bound::leq(a, 2), vec![2, 3]);
        check_watches_on(watches, Bound::leq(a, 3), vec![3]);
        check_watches_on(watches, Bound::leq(a, 4), vec![]);

        check_watches_on(watches, Bound::geq(a, 0), vec![]);
        check_watches_on(watches, Bound::geq(a, 1), vec![1]);
        check_watches_on(watches, Bound::geq(a, 2), vec![1, 2]);
        check_watches_on(watches, Bound::geq(a, 3), vec![1, 2, 3]);
        check_watches_on(watches, Bound::geq(a, 4), vec![1, 2, 3]);

        // no watches on a different variable
        check_watches_on(watches, Bound::leq(b, 0), vec![]);
        check_watches_on(watches, Bound::leq(b, 1), vec![]);
        check_watches_on(watches, Bound::leq(b, 2), vec![]);
        check_watches_on(watches, Bound::leq(b, 3), vec![]);
        check_watches_on(watches, Bound::leq(b, 4), vec![]);

        check_watches_on(watches, Bound::geq(b, 0), vec![]);
        check_watches_on(watches, Bound::geq(b, 1), vec![]);
        check_watches_on(watches, Bound::geq(b, 2), vec![]);
        check_watches_on(watches, Bound::geq(b, 3), vec![]);
        check_watches_on(watches, Bound::geq(b, 4), vec![]);
    }
}
