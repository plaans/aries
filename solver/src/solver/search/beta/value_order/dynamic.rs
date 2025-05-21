use hashbrown::HashMap;

use crate::core::state::Conflict;
use crate::core::state::Explainer;
use crate::core::Lit;
use crate::core::Relation;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::value_order::ValueOrder;
use crate::solver::search::DecLvl;

#[derive(Clone, Debug)]
pub struct Dynamic {
    table: HashMap<VarRef, i32>,
}

impl Dynamic {
    pub fn new() -> Self {
        Self { table: HashMap::new() }
    }

    /// Return the score of the given variable
    fn get(&self, var: &VarRef) -> i32 {
        *self.table.get(var).unwrap_or(&0)
    }

    /// Add the given value to the variable score.
    fn bump(&mut self, var: VarRef, value: i32) {
        if let Some(s) = self.table.get_mut(&var) {
            *s += value;
        } else {
            self.table.insert(var, value);
        }
    }

    /// Bump the variable of the given literal.
    fn handle(&mut self, lit: &Lit) {
        let var = lit.variable();
        let b = match lit.relation() {
            Relation::Gt => -1,
            Relation::Leq => 1,
        };
        self.bump(var, b);
    }
}

impl<Lbl: Label> ValueOrder<Lbl> for Dynamic {
    fn select(&self, var: VarRef, model: &Model<Lbl>) -> Lit {
        if self.get(&var) < 0 {
            let lb = model.state.lb(var);
            var.leq(lb)
        } else {
            let ub = model.state.ub(var);
            var.geq(ub)
        }
    }

    fn conflict(
        &mut self,
        clause: &Conflict,
        _model: &Model<Lbl>,
        _explainer: &mut dyn Explainer,
        _backtrack_level: DecLvl,
    ) {
        for lit in clause.literals() {
            self.handle(lit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prepare a basic model for the tests.
    /// Use it as follows.
    /// ```
    /// let (model, x, y, z) = model();
    /// ```
    ///
    /// It has three variables:
    ///  - x in \[0,7\]
    ///  - y in \[3,6\]
    ///  - z in \[-2,4\]
    fn basic_model() -> (Model<String>, VarRef, VarRef, VarRef) {
        let mut model = Model::new();
        let x = model.new_ivar(0, 7, "x").into();
        let y = model.new_ivar(3, 6, "y").into();
        let z = model.new_ivar(-2, 4, "z").into();
        (model, x, y, z)
    }

    #[test]
    fn handle() {
        let (_model, x, y, z) = basic_model();
        let mut dynamic = Dynamic::new();
        dynamic.handle(&x.geq(0));
        dynamic.handle(&y.leq(6));
        assert_eq!(dynamic.get(&x), -1);
        assert_eq!(dynamic.get(&y), 1);
        assert_eq!(dynamic.get(&z), 0);
    }

    #[test]
    fn select() {
        let (model, x, y, z) = basic_model();
        let mut dynamic = Dynamic::new();
        dynamic.bump(x, -1);
        dynamic.bump(y, 1);
        assert_eq!(dynamic.select(x, &model), x.leq(0));
        assert_eq!(dynamic.select(y, &model), y.geq(6));
        assert_eq!(dynamic.select(z, &model), z.geq(4));
    }
}
