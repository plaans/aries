use std::collections::HashMap;
use std::sync::Arc;

use aries::core::state::Domains;
use aries::core::Lit;
use aries::core::VarRef;
use aries::model::Constraint as AriesConstraint;
use aries::model::Model as AriesModel;
use aries::solver::Solver as AriesSolver;

use crate::constraint::Constraint as FznConstraint;
use crate::model::Model as FznModel;
use crate::traits::Name;
use crate::var::Var as FznVar;
use crate::var::VarBool;
use crate::var::VarInt;

pub struct Solver {
    fzn_model: FznModel,
    aries_model: AriesModel<usize>,
    translation: HashMap<usize, VarRef>,
}

impl Solver {
    pub fn new(fzn_model: FznModel) -> Self {
        let mut solver = Self::default();
        for var in fzn_model.variables() {
            solver.add_var(&var);
        }
        for constraint in fzn_model.constraints() {
            solver.add_constraint(constraint);
        }
        solver.fzn_model = fzn_model;
        solver
    }

    pub fn fzn_model(&self) -> &FznModel {
        &self.fzn_model
    }

    pub fn aries_model(&self) -> &AriesModel<usize> {
        &self.aries_model
    }

    fn add_var(&mut self, var: &FznVar) {
        match var {
            FznVar::Bool(v) => self.add_var_bool(v),
            FznVar::Int(v) => self.add_var_int(v),
            FznVar::BoolArray(_) => todo!(),
            FznVar::IntArray(_) => todo!(),
        }
    }

    fn add_var_bool(&mut self, var_bool: &VarBool) {
        let bvar = self.aries_model.new_bvar(*var_bool.id());
        self.translation.insert(*var_bool.id(), bvar.into());
    }

    fn add_var_int(&mut self, var_int: &VarInt) {
        let range = match var_int.domain() {
            crate::domain::IntDomain::Range(range) => range,
            crate::domain::IntDomain::Set(_) => todo!(),
        };
        let ivar =
            self.aries_model
                .new_ivar(*range.lb(), *range.ub(), *var_int.id());
        self.translation.insert(*var_int.id(), ivar.into());
    }

    /// Add the given flatzinc constraint to the aries model.
    fn add_constraint(&mut self, constraint: &FznConstraint) {
        let reif_expr = match constraint {
            FznConstraint::IntEq(c) => {
                let var_ref_a = self.translation.get(c.a().id()).unwrap();
                let var_ref_b = self.translation.get(c.b().id()).unwrap();
                aries::reif::ReifExpr::Eq(*var_ref_a, *var_ref_b)
            },
            FznConstraint::BoolEq(_) => todo!(),
        };
        let aries_constraint = AriesConstraint::Reified(reif_expr, Lit::TRUE);
        self.aries_model.shape.constraints.push(aries_constraint);
    }

    /// Solve the flatzinc model.
    ///
    /// For now the results are printed.
    pub fn solve(&self) -> anyhow::Result<()> {
        let mut aries_solver = AriesSolver::new(self.aries_model.clone());
        match aries_solver.solve()? {
            Some(a) => {
                for var in self.fzn_model.variables() {
                    self.print_var_value(var, &a);
                }
            }
            None => println!("UNSAT"),
        };
        Ok(())
    }

    fn print_var_value(&self, var: &FznVar, domains: &Arc<Domains>) {
        match var {
            FznVar::Bool(v) => {
                let var_ref = self.translation.get(v.id()).unwrap();
                let (lb, ub) = domains.bounds(*var_ref);
                debug_assert_eq!(lb, ub);
                println!("{} = {}", v.name(), lb);
            }
            FznVar::Int(v) => {
                let var_ref = self.translation.get(v.id()).unwrap();
                let (lb, ub) = domains.bounds(*var_ref);
                debug_assert_eq!(lb, ub);
                println!("{} = {}", v.name(), lb);
            }
            FznVar::BoolArray(_) => todo!(),
            FznVar::IntArray(_) => todo!(),
        }
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

#[cfg(test)]
mod tests {
    use crate::parser::parse_model;

    use super::*;

    #[test]
    fn basic_int() -> anyhow::Result<()> {
        const CONTENT: &str = "\
        var 0..5: x;\n\
        var 4..9: y;\n\
        constraint int_eq(x,y);\n\
        ";

        let fzn_model = parse_model(CONTENT)?;

        let solver = Solver::new(fzn_model);
        solver.solve()?;

        panic!("Force printing the result");

        Ok(())
    }
}
