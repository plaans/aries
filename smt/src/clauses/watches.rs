use aries_collections::ref_store::RefVec;
use aries_model::lang::{Bound, IntCst, VarRef};

#[derive(Debug)]
pub(crate) struct LBWatch<Watcher> {
    pub watcher: Watcher,
    pub guard: IntCst,
}

impl<Watcher> LBWatch<Watcher> {
    pub fn to_lit(&self, var: VarRef) -> Bound {
        Bound::GT(var, self.guard)
    }
}

#[derive(Debug)]
pub(crate) struct UBWatch<Watcher> {
    pub watcher: Watcher,
    pub guard: IntCst,
}

impl<Watcher> UBWatch<Watcher> {
    pub fn to_lit(&self, var: VarRef) -> Bound {
        Bound::leq(var, self.guard)
    }
}

// TODO: move to a more general location
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
        self.ensure_capacity(literal.var());

        match literal {
            Bound::LEQ(var, ub) => self.on_ub[var].push(UBWatch {
                watcher: clause,
                guard: ub,
            }),
            Bound::GT(var, below_lb) => self.on_lb[var].push(LBWatch {
                watcher: clause,
                guard: below_lb,
            }),
        }
    }

    pub(crate) fn move_lb_watches_to(&mut self, var: VarRef, out: &mut Vec<LBWatch<Watcher>>) {
        self.ensure_capacity(var);
        for watch in self.on_lb[var].drain(..) {
            out.push(watch);
        }
    }
    pub(crate) fn move_ub_watches_to(&mut self, var: VarRef, out: &mut Vec<UBWatch<Watcher>>) {
        self.ensure_capacity(var);
        for watch in self.on_ub[var].drain(..) {
            out.push(watch);
        }
    }

    pub fn is_watched_by(&self, literal: Bound, clause: Watcher) -> bool {
        match literal {
            Bound::LEQ(var, ub) => self.on_ub[var]
                .iter()
                .any(|watch| watch.watcher == clause && watch.guard <= ub),

            Bound::GT(var, below_lb) => self.on_lb[var]
                .iter()
                .any(|watch| watch.watcher == clause && watch.guard >= below_lb),
        }
    }

    pub fn remove_watch(&mut self, clause: Watcher, literal: Bound) {
        match literal {
            Bound::LEQ(var, _) => {
                let index = self.on_ub[var].iter().position(|w| w.watcher == clause).unwrap();
                self.on_ub[var].swap_remove(index);
                debug_assert!(self.on_ub[var].iter().all(|w| w.watcher != clause));
            }
            Bound::GT(var, _) => {
                let index = self.on_lb[var].iter().position(|w| w.watcher == clause).unwrap();
                self.on_lb[var].swap_remove(index);
                debug_assert!(self.on_lb[var].iter().all(|w| w.watcher != clause));
            }
        }
    }

    /// Get the constraints triggered by the literal becoming true
    /// If the literal is (n <= 4), it should trigger watches on (n <= 4), (n <= 5), ...
    /// If the literal is (n > 5), it should trigger watches on (n > 5), (n > 4), (n > 3), ...
    pub fn watches_on(&self, literal: Bound) -> Box<dyn Iterator<Item = Watcher> + '_> {
        if !self.on_ub.contains(literal.var()) {
            return Box::new(std::iter::empty());
        }
        match literal {
            Bound::LEQ(var, ub) => {
                Box::new(
                    self.on_ub[var]
                        .iter()
                        .filter_map(move |w| if w.guard >= ub { Some(w.watcher) } else { None }),
                )
            }
            Bound::GT(var, below_lb) => {
                Box::new(
                    self.on_lb[var]
                        .iter()
                        .filter_map(move |w| if w.guard < below_lb { Some(w.watcher) } else { None }),
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
