use crate::clauses::ClauseId;
use aries_collections::ref_store::RefVec;
use aries_model::lang::{Bound, IntCst, VarRef};

#[derive(Debug)]
pub(crate) struct LBWatch {
    pub watcher: ClauseId,
    pub guard: IntCst,
}

impl LBWatch {
    pub fn to_lit(&self, var: VarRef) -> Bound {
        Bound::GT(var, self.guard)
    }
}

#[derive(Debug)]
pub(crate) struct UBWatch {
    pub watcher: ClauseId,
    pub guard: IntCst,
}

impl UBWatch {
    pub fn to_lit(&self, var: VarRef) -> Bound {
        Bound::leq(var, self.guard)
    }
}

#[derive(Default)]
pub(crate) struct Watches {
    on_lb: RefVec<VarRef, Vec<LBWatch>>,
    on_ub: RefVec<VarRef, Vec<UBWatch>>,
}
impl Watches {
    fn ensure_capacity(&mut self, var: VarRef) {
        while !self.on_ub.contains(var) {
            self.on_ub.push(Vec::new());
            self.on_lb.push(Vec::new());
        }
    }

    pub fn add_watch(&mut self, clause: ClauseId, literal: Bound) {
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

    pub fn move_lb_watches_to(&mut self, var: VarRef, out: &mut Vec<LBWatch>) {
        self.ensure_capacity(var);
        for watch in self.on_lb[var].drain(..) {
            out.push(watch);
        }
    }
    pub fn move_ub_watches_to(&mut self, var: VarRef, out: &mut Vec<UBWatch>) {
        self.ensure_capacity(var);
        for watch in self.on_ub[var].drain(..) {
            out.push(watch);
        }
    }

    pub fn is_watched_by(&self, literal: Bound, clause: ClauseId) -> bool {
        match literal {
            Bound::LEQ(var, ub) => self.on_ub[var]
                .iter()
                .find(|&watch| watch.watcher == clause && watch.guard <= ub)
                .is_some(),
            Bound::GT(var, below_lb) => self.on_lb[var]
                .iter()
                .find(|&watch| watch.watcher == clause && watch.guard >= below_lb)
                .is_some(),
        }
    }

    pub fn remove_watch(&mut self, clause: ClauseId, literal: Bound) {
        match literal {
            Bound::LEQ(var, _) => {
                let index = self.on_ub[var].iter().position(|w| w.watcher == clause).unwrap();
                self.on_ub[var].swap_remove(index);
                debug_assert!(self.on_ub[var].iter().position(|w| w.watcher == clause).is_none());
            }
            Bound::GT(var, _) => {
                let index = self.on_lb[var].iter().position(|w| w.watcher == clause).unwrap();
                self.on_lb[var].swap_remove(index);
                debug_assert!(self.on_lb[var].iter().position(|w| w.watcher == clause).is_none());
            }
        }
    }

    // /// Get the constraints triggered by the literal becoming true
    // /// If the literal is (n <= 4), it should trigger watches on (n <= 4), (n <= 5), ...
    // /// If the literal is (n > 5), it should trigger watches on (n > 5), (n > 4), (n > 3), ...
    // pub fn watches_on(&self, literal: ILit) -> Box<dyn Iterator<Item = ClauseId> + '_> {
    //     if !self.on_ub.contains(literal.var()) {
    //         return Box::new(std::iter::empty());
    //     }
    //     match literal {
    //         ILit::LEQ(var, ub) => {
    //             Box::new(
    //                 self.on_ub[var]
    //                     .iter()
    //                     .filter_map(move |(cl, guard)| if *guard >= ub { Some(*cl) } else { None }),
    //             )
    //         }
    //         ILit::GT(var, below_lb) => {
    //             Box::new(
    //                 self.on_lb[var]
    //                     .iter()
    //                     .filter_map(move |(cl, guard)| if *guard < below_lb { Some(*cl) } else { None }),
    //             )
    //         }
    //     }
    // }
}
