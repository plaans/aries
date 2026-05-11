use std::{collections::HashMap, sync::Arc};

use aries::prelude::{Lit, VarRef};
use smallvec::{SmallVec, smallvec};

use crate::{LpCol, LpLit};

pub type LitToLpLitsBindingFn = dyn Fn(Lit) -> SmallVec<[LpLit; 4]> + Send + Sync;
pub type LpLitToLitsBindingFn = dyn Fn(LpLit) -> SmallVec<[Lit; 4]> + Send + Sync;

#[derive(Clone, Default)]
pub(crate) struct LpRelaxBindings {
    lit_to_lplits_bindings: HashMap<VarRef, LitToLpLitsBindings>,
    lplit_to_lits_bindings: HashMap<LpCol, LpLitToLitsBindings>,
}
impl LpRelaxBindings {
    pub fn add_lit_to_lplits_binding(&mut self, var: VarRef, func: Arc<LitToLpLitsBindingFn>) {
        self.lit_to_lplits_bindings
            .entry(var)
            .or_insert_with(|| LitToLpLitsBindings::new(var))
            .add(func);
    }
    pub fn add_lplit_to_lits_binding(&mut self, col: LpCol, func: Arc<LpLitToLitsBindingFn>) {
        self.lplit_to_lits_bindings
            .entry(col)
            .or_insert_with(|| LpLitToLitsBindings::new(col))
            .add(func);
    }
    pub fn add_lit_to_lplits_binding_default(&mut self, var: VarRef, col: LpCol) {
        self.lit_to_lplits_bindings
            .entry(var)
            .or_insert_with(|| LitToLpLitsBindings::new(var))
            .add_default(var, col);
    }
    pub fn add_lplit_to_lits_binding_default(&mut self, var: VarRef, col: LpCol) {
        self.lplit_to_lits_bindings
            .entry(col)
            .or_insert_with(|| LpLitToLitsBindings::new(col))
            .add_default(var, col);
    }
    pub fn compute_implied_lplits(&self, lit: Lit) -> Option<impl Iterator<Item = LpLit>> {
        self.lit_to_lplits_bindings
            .get(&lit.variable())
            .map(|bindings| bindings.compute_implied_lplits(lit))
    }
    pub fn compute_implied_lits(&self, lplit: LpLit) -> Option<impl Iterator<Item = Lit>> {
        self.lplit_to_lits_bindings
            .get(&lplit.col)
            .map(|bindings| bindings.compute_implied_lit(lplit))
    }
}

#[derive(Clone)]
struct LitToLpLitsBindings {
    var: VarRef,
    funcs: Vec<Arc<LitToLpLitsBindingFn>>,
}
impl LitToLpLitsBindings {
    fn new(var: VarRef) -> Self {
        Self { var, funcs: vec![] }
    }
    fn add_default(&mut self, var: VarRef, col: LpCol) {
        assert!(var == self.var);
        self.add(Arc::new(move |lit| smallvec![LpLit::from_model_lit(col, lit)]))
    }
    fn add(&mut self, func: Arc<LitToLpLitsBindingFn>) {
        self.funcs.push(func);
    }
    fn compute_implied_lplits(&self, lit: Lit) -> impl Iterator<Item = LpLit> + use<'_> {
        assert!(lit.variable() == self.var);
        self.funcs.iter().flat_map(move |func| func(lit))
    }
}

#[derive(Clone)]
struct LpLitToLitsBindings {
    col: LpCol,
    funcs: Vec<Arc<LpLitToLitsBindingFn>>,
}
impl LpLitToLitsBindings {
    fn new(col: LpCol) -> Self {
        Self { col, funcs: vec![] }
    }
    fn add_default(&mut self, var: VarRef, col: LpCol) {
        assert!(col == self.col);
        self.add(Arc::new(move |lplit| smallvec![lplit.into_model_lit(var)]))
    }
    fn add(&mut self, func: Arc<LpLitToLitsBindingFn>) {
        self.funcs.push(func);
    }
    fn compute_implied_lit(&self, lplit: LpLit) -> impl Iterator<Item = Lit> + use<'_> {
        assert!(lplit.col == self.col);
        self.funcs.iter().flat_map(move |func| func(lplit))
    }
}
