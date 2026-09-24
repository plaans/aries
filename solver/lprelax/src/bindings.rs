use std::sync::Arc;

use aries_solver::core::views::{Dom, VarView};

use idmap::DirectIdMap;

use crate::types::*;

pub type AriesLitToLpLitHalfBindingFn = dyn Fn(IntCst) -> Option<(LpLitType, IntCst)>;

#[derive(Default, Clone)]
pub(super) struct AriesLitToLpLitHalfBindings {
    on_scope_var: DirectIdMap<AriesVarIdx, (Vec<AriesLitToLpLitHalfBinding>, Vec<AriesLitToLpLitHalfBinding>)>,
    on_var: DirectIdMap<AriesVarIdx, (Vec<AriesLitToLpLitHalfBinding>, Vec<AriesLitToLpLitHalfBinding>)>,
}
impl AriesLitToLpLitHalfBindings {
    pub fn add(&mut self, binding: AriesLitToLpLitHalfBinding) {
        let scope_var_idx = binding.scope.variable().to_u32();
        let var_idx = binding.svar.variable().to_u32();

        if !self.on_scope_var.contains_key(scope_var_idx) {
            self.on_scope_var.insert(scope_var_idx, Default::default());
        }
        if !self.on_var.contains_key(var_idx) {
            self.on_var.insert(var_idx, Default::default());
        }

        {
            if binding.scope.svar().is_plus() {
                &mut self.on_scope_var[scope_var_idx].1
            } else {
                &mut self.on_scope_var[scope_var_idx].0
            }
        }
        .push(binding.clone());

        {
            if binding.svar.is_plus() {
                &mut self.on_var[var_idx].1
            } else {
                &mut self.on_var[var_idx].0
            }
        }
        .push(binding.clone());
    }

    pub fn eval_for_scope_var(
        &self,
        scope_svar: AriesSignedVar,
        dom: &impl Dom,
    ) -> impl Iterator<Item = (LpLit, AriesLit, AriesLit)> {
        self.on_scope_var
            .get(scope_svar.variable().to_u32())
            .map(|(neg, pos)| if scope_svar.is_plus() { pos } else { neg })
            .into_iter()
            .flatten()
            .filter_map(move |binding| binding.eval(dom))
    }

    pub fn eval_for_var(
        &self,
        svar: AriesSignedVar,
        dom: &impl Dom,
    ) -> impl Iterator<Item = (LpLit, AriesLit, AriesLit)> {
        self.on_var
            .get(svar.variable().to_u32())
            .map(|(neg, pos)| if svar.is_plus() { pos } else { neg })
            .into_iter()
            .flatten()
            .filter_map(move |binding| binding.eval(dom))
    }

    pub fn eval_all(&self, dom: &impl Dom) -> impl Iterator<Item = (LpLit, AriesLit, AriesLit)> {
        self.on_scope_var
            .iter()
            .flat_map(|(_, (neg, pos))| neg.iter().chain(pos.iter()))
            .filter_map(move |binding| binding.eval(dom))
    }
}

#[derive(Clone)]
pub(super) struct AriesLitToLpLitHalfBinding {
    scope: AriesLit,
    svar: AriesSignedVar,
    col: LpCol,
    map_fn: Arc<AriesLitToLpLitHalfBindingFn>,
}
impl AriesLitToLpLitHalfBinding {
    pub fn new(scope: AriesLit, svar: AriesSignedVar, col: LpCol, map_fn: Arc<AriesLitToLpLitHalfBindingFn>) -> Self {
        assert!(scope != AriesLit::FALSE);
        assert!(svar.variable() != AriesVar::ZERO);
        Self {
            scope,
            svar,
            col,
            map_fn,
        }
    }
    pub fn eval(&self, dom: &impl Dom) -> Option<(LpLit, AriesLit, AriesLit)> {
        if !dom.entails(self.scope) {
            return None;
        }
        let aries_var_bound_value = if self.svar.is_plus() {
            dom.ub(self.svar.variable())
        } else {
            dom.lb(self.svar.variable())
        };
        let (tpe, val) = (self.map_fn)(aries_var_bound_value)?;
        Some((
            LpLit::new(self.col, tpe, val),
            self.scope,
            AriesLit::new(self.svar, self.svar.upper_bound(dom)),
        ))
    }
}
