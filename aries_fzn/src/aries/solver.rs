use std::collections::HashMap;

use aries::core::IntCst;
use aries::core::VarRef;
use aries::core::state::Domains;
use aries::model::Model as AriesModel;
use aries::solver::Exit;
use aries::solver::Solver as AriesSolver;
use aries::solver::search::default_brancher;

use crate::aries::Brancher;
use crate::aries::Post;
use crate::aries::constraint::InSet;
use crate::fzn::constraint::Constraint as FznConstraint;
use crate::fzn::constraint::Encode;
use crate::fzn::domain::BoolDomain;
use crate::fzn::domain::IntDomain;
use crate::fzn::model::Model as FznModel;
use crate::fzn::solution::Assignment;
use crate::fzn::solution::Solution;
use crate::fzn::solve::Goal;
use crate::fzn::solve::SolveItem;
use crate::fzn::types::as_int;
use crate::fzn::var::Var as FznVar;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;

/// Flatzinc solver using aries.
///
/// It is responsible to translate flatzinc objects into aries objects.
pub struct Solver {
    fzn_model: FznModel,
    aries_model: AriesModel<usize>,
    translation: HashMap<usize, VarRef>,
    brancher: Brancher,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            fzn_model: Default::default(),
            aries_model: Default::default(),
            translation: Default::default(),
            brancher: default_brancher(),
        }
    }
}

impl Solver {
    pub fn new(fzn_model: FznModel) -> Self {
        let mut solver = Self::default();
        for var in fzn_model.variables() {
            solver.add_var(var);
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

    pub fn set_brancher(&mut self, brancher: Brancher) {
        self.brancher = brancher;
    }

    fn add_var(&mut self, var: &FznVar) {
        match var {
            FznVar::Bool(v) => self.add_var_bool(v),
            FznVar::Int(v) => self.add_var_int(v),
            FznVar::BoolArray(_) => { /* Do nothing, variables already added */
            }
            FznVar::IntArray(_) => { /* Do nothing, variables already added */ }
        }
    }

    fn add_var_bool(&mut self, var_bool: &VarBool) {
        let ivar = match var_bool.domain() {
            BoolDomain::Singleton(b) => {
                let x = as_int(*b);
                self.aries_model.new_ivar(x, x, *var_bool.id())
            }
            BoolDomain::Both => {
                self.aries_model.new_bvar(*var_bool.id()).int_view()
            }
        };
        self.translation.insert(*var_bool.id(), ivar.into());
    }

    fn add_var_int(&mut self, var_int: &VarInt) {
        let ivar = match var_int.domain() {
            IntDomain::Range(range) => {
                let (lb, ub) = range.bounds();
                self.aries_model.new_ivar(*lb, *ub, *var_int.id())
            }
            IntDomain::Singleton(x) => {
                self.aries_model.new_ivar(*x, *x, *var_int.id())
            }
            IntDomain::Set(set) => {
                let (lb, ub) = set.bounds();
                let ivar = self.aries_model.new_ivar(*lb, *ub, *var_int.id());
                let in_set = InSet::new(ivar, set.values().clone());
                in_set.post(&mut self.aries_model);
                ivar
            }
        };
        self.translation.insert(*var_int.id(), ivar.into());
    }

    /// Add the given flatzinc constraint to the aries model.
    fn add_constraint(&mut self, constraint: &FznConstraint) {
        constraint
            .encode(&self.translation)
            .post(&mut self.aries_model);
    }

    /// Create and configure aries solver.
    fn aries_solver(&self) -> AriesSolver<usize> {
        let mut aries_solver = AriesSolver::new(self.aries_model.clone());
        aries_solver.brancher = self.brancher.clone_to_box();
        aries_solver
    }

    /// Solve the flatzinc model.
    ///
    /// If the problem is satisfiable, the aries domains are mapped to assignments.
    pub fn solve(&self) -> Result<Option<Solution>, Exit> {
        let mut aries_solver = self.aries_solver();

        match self.fzn_model.solve_item() {
            SolveItem::Satisfy => {
                let res = match aries_solver.solve()? {
                    Some(domains) => {
                        let solution = self.make_solution(&domains);
                        Some(solution)
                    }
                    None => None,
                };
                Ok(res)
            }
            SolveItem::Optimize(objective) => {
                let obj_var = objective.variable();
                let obj_var_ref = *self.translation.get(obj_var.id()).unwrap();
                let is_minimize = objective.goal() == &Goal::Minimize;
                let aries_res = if is_minimize {
                    aries_solver.minimize(obj_var_ref)?
                } else {
                    aries_solver.maximize(obj_var_ref)?
                };
                let res = match aries_res {
                    Some((_, domains)) => {
                        let solution = self.make_solution(&domains);
                        Some(solution)
                    }
                    None => None,
                };
                Ok(res)
            }
        }
    }

    /// Solve the flatzinc model with a call back for new solution.
    /// Return `true` iff the solver found a solution.
    pub fn solve_with<F>(&self, mut f: F) -> anyhow::Result<bool>
    where
        F: FnMut(Solution),
    {
        let mut aries_solver = self.aries_solver();
        let translate = |vid| *self.translation.get(vid).unwrap();

        match self.fzn_model.solve_item() {
            SolveItem::Satisfy => {
                let output_var_ids = self.output_var_ids();
                let var_refs: Vec<VarRef> =
                    output_var_ids.iter().map(translate).collect();

                let g = |d: &Domains| f(self.make_solution(d));
                let sat = aries_solver.enumerate_with(&var_refs, g)?;
                Ok(sat)
            }
            SolveItem::Optimize(objective) => {
                let obj_var = objective.variable();
                let obj_var_ref = *self.translation.get(obj_var.id()).unwrap();
                let is_minimize = objective.goal() == &Goal::Minimize;
                let g = |_: IntCst, d: &Domains| f(self.make_solution(d));
                let sat = if is_minimize {
                    aries_solver
                        .minimize_with_callback(obj_var_ref, g)?
                        .is_some()
                } else {
                    aries_solver
                        .maximize_with_callback(obj_var_ref, g)?
                        .is_some()
                };
                Ok(sat)
            }
        }
    }

    /// Return all flatzinc var ids marked as output.
    fn output_var_ids(&self) -> Vec<usize> {
        let mut var_ids = Vec::new();
        for var in self.fzn_model.variables() {
            if !var.output() {
                continue;
            }
            match var {
                FznVar::Bool(v) => var_ids.push(*v.id()),
                FznVar::Int(v) => var_ids.push(*v.id()),
                FznVar::BoolArray(v) => {
                    var_ids.extend(v.variables().map(|v| *v.id()))
                }
                FznVar::IntArray(v) => {
                    var_ids.extend(v.variables().map(|v| *v.id()))
                }
            }
        }
        var_ids
    }

    fn make_assignment(&self, var: &FznVar, domains: &Domains) -> Assignment {
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
            FznVar::BoolArray(var) => {
                let mut values = Vec::new();
                for v in var.variables() {
                    let var_ref = self.translation.get(v.id()).unwrap();
                    let (lb, ub) = domains.bounds(*var_ref);
                    debug_assert_eq!(lb, ub);
                    debug_assert!(lb == 0 || lb == 1);
                    let value = lb == 1;
                    values.push(value);
                }
                Assignment::BoolArray(var.clone(), values)
            }
            FznVar::IntArray(var) => {
                let mut values = Vec::new();
                for v in var.variables() {
                    let var_ref = self.translation.get(v.id()).unwrap();
                    let (lb, ub) = domains.bounds(*var_ref);
                    debug_assert_eq!(lb, ub);
                    values.push(lb);
                }
                Assignment::IntArray(var.clone(), values)
            }
        }
    }

    /// Make solution from flatzinc variables.
    fn make_solution(&self, domains: &Domains) -> Solution {
        let assignments = self
            .fzn_model
            .variables()
            .filter(|v| v.output())
            .map(|v| self.make_assignment(v, domains))
            .collect();
        Solution::new(assignments)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::fzn::constraint::builtins::BoolEq;
    use crate::fzn::constraint::builtins::IntEq;
    use crate::fzn::constraint::builtins::IntNe;
    use crate::fzn::domain::BoolDomain;
    use crate::fzn::domain::IntRange;
    use crate::fzn::model::Model;
    use crate::fzn::types::Int;

    use super::*;

    #[test]
    fn basic_unsat() -> anyhow::Result<()> {
        let mut fzn_model = Model::new();

        let domain_x = IntRange::new(0, 3)?;
        let domain_y = IntRange::new(4, 5)?;

        let x =
            fzn_model.new_var_int(domain_x.into(), "x".to_string(), true)?;
        let y =
            fzn_model.new_var_int(domain_y.into(), "y".to_string(), true)?;

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

        let domain_x = IntRange::new(0, 3)?.into();
        let domain_y = IntRange::new(3, 5)?.into();

        let x = fzn_model.new_var_int(domain_x, "x".to_string(), true)?;
        let y = fzn_model.new_var_int(domain_y, "y".to_string(), true)?;

        let int_eq = IntEq::new(x.clone(), y.clone());

        fzn_model.add_constraint(int_eq.into());

        let solver = Solver::new(fzn_model);
        let solution = solver.solve()?.expect("should not be unsat");

        let assignment_x = Assignment::Int(x, 3);
        let assignment_y = Assignment::Int(y, 3);

        assert!(solution.assignments().contains(&assignment_x));
        assert!(solution.assignments().contains(&assignment_y));

        Ok(())
    }

    #[test]
    fn bool_eq() -> anyhow::Result<()> {
        let mut fzn_model = Model::new();

        let x =
            fzn_model.new_var_bool(BoolDomain::Both, "x".to_string(), true)?;
        let y =
            fzn_model.new_var_bool(BoolDomain::Both, "y".to_string(), true)?;

        let bool_eq = BoolEq::new(x.clone(), y.clone());

        fzn_model.add_constraint(bool_eq.into());

        let solver = Solver::new(fzn_model);
        let solution = solver.solve()?.expect("should not be unsat");

        assert_eq!(solution.assignments().len(), 2);

        let x_true = Assignment::Bool(x.clone(), true);
        let y_true = Assignment::Bool(y.clone(), true);
        let x_false = Assignment::Bool(x, false);
        let y_false = Assignment::Bool(y, false);

        let both_true = solution.assignments().contains(&x_true)
            && solution.assignments().contains(&y_true);

        let both_false = solution.assignments().contains(&x_false)
            && solution.assignments().contains(&y_false);

        assert!(both_true || both_false);

        Ok(())
    }

    #[test]
    fn max_solve_with() -> anyhow::Result<()> {
        let mut fzn_model = FznModel::new();

        let domain_x = IntRange::new(1, 9)?.into();
        let domain_y = IntRange::new(2, 8)?.into();
        let x = fzn_model.new_var_int(domain_x, "x".to_string(), true)?;
        let y = fzn_model.new_var_int(domain_y, "y".to_string(), true)?;
        fzn_model.maximize(x.clone()).unwrap();

        fzn_model.add_constraint(IntNe::new(x.clone(), y.clone()).into());

        let solver = Solver::new(fzn_model);

        let mut solutions = Vec::new();
        let f = |solution| solutions.push(solution);
        solver.solve_with(f).unwrap();

        // Check objective is increasing
        let mut objective = Int::MIN;
        for solution in solutions {
            // Assume x is in position 0
            let (x_sol, value): (Rc<VarInt>, Int) =
                solution.assignments()[0].clone().try_into().unwrap();
            assert_eq!(x_sol, x);
            assert!(value >= objective);
            objective = value;
        }

        Ok(())
    }
}
