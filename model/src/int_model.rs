use crate::lang::{DVar, IVar, IntCst};
use crate::{Label, WriterId};
use aries_backtrack::Q;
use aries_backtrack::{Backtrack, BacktrackWith};
use aries_collections::ref_store::RefVec;

#[derive(Clone)]
pub struct IntDomain {
    pub lb: IntCst,
    pub ub: IntCst,
}
impl IntDomain {
    pub fn new(lb: IntCst, ub: IntCst) -> IntDomain {
        IntDomain { lb, ub }
    }
}
pub struct VarEvent {
    pub var: DVar,
    pub ev: DomEvent,
}
pub enum DomEvent {
    NewLB { prev: IntCst, new: IntCst },
    NewUB { prev: IntCst, new: IntCst },
}

#[derive(Default)]
pub struct IntModel {
    labels: RefVec<DVar, Label>,
    pub(crate) domains: RefVec<DVar, IntDomain>,
    trail: Q<(VarEvent, WriterId)>,
}

impl IntModel {
    pub fn new() -> IntModel {
        IntModel {
            labels: Default::default(),
            domains: Default::default(),
            trail: Default::default(),
        }
    }

    pub fn new_ivar<L: Into<Label>>(&mut self, lb: IntCst, ub: IntCst, label: L) -> DVar {
        let id1 = self.labels.push(label.into());
        let id2 = self.domains.push(IntDomain::new(lb, ub));
        debug_assert_eq!(id1, id2);
        id1
    }

    pub fn variables(&self) -> impl Iterator<Item = DVar> {
        self.labels.keys()
    }

    pub fn label(&self, var: impl Into<DVar>) -> Option<&str> {
        self.labels[var.into()].get()
    }

    pub fn domain_of(&self, var: impl Into<DVar>) -> &IntDomain {
        &self.domains[var.into()]
    }

    fn dom_mut(&mut self, var: impl Into<DVar>) -> &mut IntDomain {
        &mut self.domains[var.into()]
    }

    pub fn set_lb(&mut self, var: IVar, lb: IntCst, writer: WriterId) {
        let dom = self.dom_mut(var);
        let prev = dom.lb;
        if prev < lb {
            dom.lb = lb;
            let event = VarEvent {
                var: var.into(),
                ev: DomEvent::NewLB { prev, new: lb },
            };
            self.trail.push((event, writer));
        }
    }

    pub fn set_ub(&mut self, var: IVar, ub: IntCst, writer: WriterId) {
        let dom = self.dom_mut(var);
        let prev = dom.ub;
        if prev > ub {
            dom.ub = ub;
            let event = VarEvent {
                var: var.into(),
                ev: DomEvent::NewUB { prev, new: ub },
            };
            self.trail.push((event, writer));
        }
    }

    fn undo_event(domains: &mut RefVec<DVar, IntDomain>, ev: VarEvent) {
        let dom = &mut domains[ev.var];
        match ev.ev {
            DomEvent::NewLB { prev, new } => {
                debug_assert_eq!(dom.lb, new);
                dom.lb = prev;
            }
            DomEvent::NewUB { prev, new } => {
                debug_assert_eq!(dom.ub, new);
                dom.ub = prev;
            }
        }
    }
}

impl Backtrack for IntModel {
    fn save_state(&mut self) -> u32 {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        let domains = &mut self.domains;
        self.trail.restore_last_with(|(ev, _)| Self::undo_event(domains, ev));
    }

    fn restore(&mut self, saved_id: u32) {
        let domains = &mut self.domains;
        self.trail
            .restore_with(saved_id, |(ev, _)| Self::undo_event(domains, ev));
    }
}
