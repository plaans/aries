use aries_backtrack::Trail;
use aries_collections::ref_store::RefVec;
use aries_collections::*;
use aries_model::lang::{IVar, IntCst};
use aries_model::WModel;
use aries_sat::all::Lit;

use std::collections::HashMap;

create_ref_type!(CId);

pub enum Change {
    Lb(IVar),
    Ub(IVar),
}

#[derive(Default)]
struct VarMeta {
    watches: Vec<CId>,
}

#[derive(Debug)]
pub enum UpdateFail {
    EmptyDom(IVar),
}
pub type Update = std::result::Result<(), UpdateFail>;

enum Event {
    AddedWatch(IVar),
    // removed a watch of the variable by the constraint.
    // the watch was positioned at the given index
    RemovedWatch(IVar, CId, usize),
}

#[derive(Default)]
pub struct CSP {
    immut: Immut,
    dyna: Dyna,
}

#[derive(Default)]
pub struct Dyna {
    meta: HashMap<IVar, VarMeta>,
    trail: Trail<Event>,
}
impl Dyna {
    fn view_for<'a, 'b>(&'a mut self, cid: CId, model: WModel<'b>) -> CSPView<'a, 'b> {
        CSPView {
            owner: cid,
            csp: self,
            model,
        }
    }

    fn meta(&mut self, var: IVar) -> &mut VarMeta {
        if !self.meta.contains_key(&var) {
            self.meta.insert(var, VarMeta::default());
        }
        self.meta.get_mut(&var).unwrap()
    }

    pub fn watch(&mut self, cid: CId, var: IVar) {
        self.meta(var).watches.push(cid);
        self.trail.push(Event::AddedWatch(var));
    }
    pub fn unwatch(&mut self, cid: CId, var: IVar) {
        let watches = &mut self.meta(var).watches;
        let pos = watches.iter().position(|&c| c == cid).unwrap();
        watches.remove(pos);
        self.trail.push(Event::RemovedWatch(var, cid, pos));
    }
    pub fn clear_watches(&mut self, _cid: CId) {
        todo!()
    }
}
#[derive(Default)]
struct Immut {
    activations: HashMap<Lit, Vec<CId>>,
    constraints: RefVec<CId, Box<dyn Constraint>>,
    triggers: RefVec<CId, Lit>,
}

impl CSP {
    pub fn record(&mut self, trigger: Lit, constraint: Box<dyn Constraint>) -> CId {
        let cid = self.immut.constraints.push(constraint);
        if !self.immut.activations.contains_key(&trigger) {
            self.immut.activations.insert(trigger, vec![cid]);
        } else {
            self.immut.activations.get_mut(&trigger).unwrap().push(cid);
        }
        let cid2 = self.immut.triggers.push(trigger);
        debug_assert_eq!(cid, cid2);
        cid
    }

    pub fn trigger(&mut self, lit: Lit, mut model: WModel) -> Update {
        for &cid in self.immut.activations.get(&lit).unwrap_or(&Vec::new()) {
            let c = &self.immut.constraints[cid];
            c.init(self.dyna.view_for(cid, model.dup()))?;
        }
        Ok(())
    }

    pub fn propagate(&mut self, changed: IVar, mut model: WModel) -> Update {
        let watches = self.dyna.meta[&changed].watches.clone(); // todo: inefficient
        for cid in watches {
            let c = &self.immut.constraints[cid];
            let view = self.dyna.view_for(cid, model.dup());
            c.propagate(changed, view)?;
        }
        Ok(())
    }
}

pub struct CSPView<'a, 'b> {
    owner: CId,
    csp: &'a mut Dyna,
    model: WModel<'b>,
}
impl<'a, 'b> CSPView<'a, 'b> {
    pub fn make_passive(&mut self) {
        todo!()
    }

    pub fn watch(&mut self, ivar: IVar) {
        self.csp.watch(self.owner, ivar);
    }
    pub fn unwatch(&mut self, ivar: IVar) {
        self.csp.unwatch(self.owner, ivar);
    }
    pub fn clear_watches(&mut self) {
        self.csp.clear_watches(self.owner)
    }

    pub fn bounds(&self, ivar: IVar) -> (IntCst, IntCst) {
        self.model.bounds(ivar)
    }
    pub fn lb(&self, ivar: IVar) -> IntCst {
        self.model.bounds(ivar).0
    }
    pub fn ub(&self, ivar: IVar) -> IntCst {
        self.model.bounds(ivar).1
    }
    pub fn is_instantiated(&self, ivar: IVar) -> bool {
        let (lb, ub) = self.model.bounds(ivar);
        lb == ub
    }

    pub fn set_lb(&mut self, ivar: IVar, lb: IntCst) -> Result<bool, UpdateFail> {
        let (prev_lb, ub) = self.model.bounds(ivar);
        if lb > ub {
            Err(UpdateFail::EmptyDom(ivar))
        } else if prev_lb < lb {
            self.model.set_lower_bound(ivar, lb);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn set_ub(&mut self, ivar: IVar, ub: IntCst) -> Result<bool, UpdateFail> {
        let (lb, prev_ub) = self.model.bounds(ivar);
        if lb > ub {
            Err(UpdateFail::EmptyDom(ivar))
        } else if ub < prev_ub {
            self.model.set_upper_bound(ivar, ub);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

pub trait Constraint {
    fn init(&self, csp: CSPView) -> Update;

    fn propagate(&self, changed: IVar, csp: CSPView) -> Update;

    fn explain_lb(&self, ivar: IVar, out: &mut Vec<Change>);
}
