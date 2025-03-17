use std::collections::HashMap;

use aries::core::VarRef as VarRef;
use aries::model::Model as AriesModel;

use crate::model::Model as FznModel;
use crate::traits::Name;
use crate::var::Var as FznVar;
use crate::var::VarBool;
use crate::var::VarInt;

pub struct Solver {
    fzn_model: FznModel,
    aries_model: AriesModel<String>,
    translation: HashMap<String, VarRef>
}

impl Solver {
    pub fn new(fzn_model: FznModel) -> Self {
        let mut solver = Self::default();
        for var in fzn_model.variables() {
            solver.add_var(&var);
        }
        solver
    }

    pub fn fzn_model(&self) -> &FznModel {
        &self.fzn_model
    }

    pub fn aries_model(&self) -> &AriesModel<String> {
        &self.aries_model
    }

    pub fn add_var(&mut self, var: &FznVar) {
        match var {
            FznVar::Bool(v) => self.add_var_bool(v),
            FznVar::Int(v) => self.add_var_int(v),
            FznVar::BoolArray(_) => todo!(),
            FznVar::IntArray(_) => todo!(),
        }
    }

    pub fn add_var_bool(&mut self, var_bool: &VarBool) {
        let bvar = self.aries_model.new_bvar(var_bool.name().as_ref().unwrap());
        self.translation.insert(var_bool.name().clone().unwrap(), bvar.into());
    }

    pub fn add_var_int(&mut self, var_int: &VarInt) {
        let range = match var_int.domain() {
            crate::domain::IntDomain::Range(range) => range,
            crate::domain::IntDomain::Set(_) => todo!(),
        };
        let ivar = self.aries_model.new_ivar(*range.lb(), *range.ub(), var_int.name().clone().unwrap());
        self.translation.insert(var_int.name().clone().unwrap(), ivar.into());
    }
}

impl Default for Solver {
    fn default() -> Self {
        Self { 
            fzn_model: Default::default(),
            aries_model: Default::default(),
            translation: Default::default(),
        }
    }
}