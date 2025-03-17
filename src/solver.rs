use std::collections::HashMap;
use std::sync::Arc;

use aries::core::state::Domains;
use aries::core::Lit;
use aries::core::VarRef;
use aries::model::Constraint as AriesConstraint;
use aries::model::Model as AriesModel;
use aries::solver::Exit;
use aries::solver::Solver as AriesSolver;

use crate::constraint::Constraint as FznConstraint;
use crate::model::Model as FznModel;
use crate::var::Assignment;
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
            }
            FznConstraint::BoolEq(c) => {
                let var_ref_a = self.translation.get(c.a().id()).unwrap();
                let var_ref_b = self.translation.get(c.b().id()).unwrap();
                aries::reif::ReifExpr::Eq(*var_ref_a, *var_ref_b)
            }
        };
        let aries_constraint = AriesConstraint::Reified(reif_expr, Lit::TRUE);
        self.aries_model.shape.constraints.push(aries_constraint);
    }

    /// Solve the flatzinc model.
    ///
    /// For now the results are printed.
    pub fn solve(&self) -> Result<Option<Vec<Assignment>>, Exit> {
        let mut aries_solver = AriesSolver::new(self.aries_model.clone());

        let res = match aries_solver.solve()? {
            Some(a) => {
                let mut assignments = Vec::new();
                for var in self.fzn_model.variables() {
                    let assignment = self.make_assignment(var, &a);
                    assignments.push(assignment);
                }
                Some(assignments)
            }
            None => None,
        };
        Ok(res)
    }

    fn make_assignment(
        &self,
        var: &FznVar,
        domains: &Arc<Domains>,
    ) -> Assignment {
        match var {
            FznVar::Bool(v) => {
                let var_ref = self.translation.get(v.id()).unwrap();
                let (lb, ub) = domains.bounds(*var_ref);
                debug_assert_eq!(lb, ub);
                debug_assert!(lb == 0 || lb == 1);
                let value = lb == 1;
                Assignment::Bool(v.clone(), value)
            }
            FznVar::Int(v) => {
                let var_ref = self.translation.get(v.id()).unwrap();
                let (lb, ub) = domains.bounds(*var_ref);
                debug_assert_eq!(lb, ub);
                Assignment::Int(v.clone(), lb)
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
    use crate::constraint::builtins::BoolEq;
    use crate::constraint::builtins::IntEq;
    use crate::domain::BoolDomain;
    use crate::domain::IntRange;
    use crate::model::Model;

    use super::*;

    #[test]
    fn basic_unsat() -> anyhow::Result<()> {
        let mut fzn_model = Model::new();

        let domain_x = IntRange::new(0, 3)?;
        let domain_y = IntRange::new(4, 5)?;

        let x = fzn_model.new_var_int(domain_x.into(), "x".to_string())?;
        let y = fzn_model.new_var_int(domain_y.into(), "y".to_string())?;

        let int_eq = IntEq::new(x, y);

        fzn_model.add_constraint(int_eq.into());

        let solver = Solver::new(fzn_model);
        let result = solver.solve()?;

        assert_eq!(result, None);

        Ok(())
    }

    #[test]
    fn int_eq() -> anyhow::Result<()> {
        let mut fzn_model = Model::new();

        let domain_x = IntRange::new(0, 3)?;
        let domain_y = IntRange::new(3, 5)?;

        let x = fzn_model.new_var_int(domain_x.into(), "x".to_string())?;
        let y = fzn_model.new_var_int(domain_y.into(), "y".to_string())?;

        let int_eq = IntEq::new(x.clone(), y.clone());

        fzn_model.add_constraint(int_eq.into());

        let solver = Solver::new(fzn_model);
        let assignments = solver.solve()?.expect("should not be unsat");

        let assignment_x = Assignment::Int(x, 3);
        let assignment_y = Assignment::Int(y, 3);

        assert!(assignments.contains(&assignment_x));
        assert!(assignments.contains(&assignment_y));

        Ok(())
    }

    #[test]
    fn bool_eq() -> anyhow::Result<()> {
        let mut fzn_model = Model::new();

        let x = fzn_model.new_var_bool(BoolDomain, "x".to_string())?;
        let y = fzn_model.new_var_bool(BoolDomain, "y".to_string())?;

        let bool_eq = BoolEq::new(x.clone(), y.clone());

        fzn_model.add_constraint(bool_eq.into());

        let solver = Solver::new(fzn_model);
        let assignments = solver.solve()?.expect("should not be unsat");

        assert_eq!(assignments.len(), 2);

        let x_true = Assignment::Bool(x.clone(), true);
        let y_true = Assignment::Bool(y.clone(), true);
        let x_false = Assignment::Bool(x, false);
        let y_false = Assignment::Bool(y, false);

        let both_true =
            assignments.contains(&x_true) && assignments.contains(&y_true);

        let both_false =
            assignments.contains(&x_false) && assignments.contains(&y_false);

        assert!(both_true || both_false);

        Ok(())
    }
}
