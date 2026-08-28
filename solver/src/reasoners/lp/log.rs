use std::fs::File;
use std::io::BufWriter;

use serde::{Deserialize, Serialize};

use minilp::{Bound, Problem, Variable};

/// Store all the information relative to a set bound in minilp
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct BoundSetEvent {
    var: Variable,
    bound: Bound,
    val: f64,
}

/// Contains all the bound changes on lp variables that occur during the execution of the solver
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StackEvent {
    stack: Vec<BoundSetEvent>,
}

impl StackEvent {
    pub fn new() -> Self {
        StackEvent { stack: Vec::new() }
    }

    pub fn push_event(&mut self, var: Variable, bound: Bound, val: f64) {
        self.stack.push(BoundSetEvent { var, bound, val });
    }

    pub fn iter(&self) -> impl Iterator<Item = &BoundSetEvent> {
        self.stack.iter()
    }
}

/// Wrap the original problem with no constraint activated with its execution stack
///
/// Used to store all the necessary information for re-execution at once in a file
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Logger {
    pub(super) problem: Problem,
    pub(super) stack_event: StackEvent,
}

impl Logger {
    pub fn new() -> Self {
        Logger {
            problem: Problem::new(minilp::OptimizationDirection::Minimize),
            stack_event: StackEvent::new(),
        }
    }

    pub fn set_problem(&mut self, problem: Problem) {
        self.problem = problem;
    }

    pub fn save_to(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);

        postcard::to_io(self, writer)?;
        Ok(())
    }

    pub fn load_from(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path)?;

        let logger: Logger = postcard::from_bytes(&bytes)?;

        Ok(logger)
    }
}

#[cfg(test)]
mod tests {

    use good_lp::{Expression, ProblemVariables, Solver, SolverModel, constraint, highs};
    use itertools::Itertools;
    use minilp::{ComparisonOp, OptimizationDirection};

    use super::*;

    pub fn build_good_lp_model<S: Solver>(
        problem: &Problem,
        solver: S,
    ) -> Result<impl SolverModel, Box<dyn std::error::Error>> {
        let mut vars = ProblemVariables::default();

        // 1. Instanciation of the good lp variables
        let good_vars: Vec<good_lp::Variable> = problem
            .get_var_mins()
            .iter()
            .zip(problem.get_var_maxs())
            .map(|(&lb, &ub)| {
                let mut v = good_lp::variable();
                if lb != -f64::INFINITY {
                    v = v.min(lb);
                }
                if ub != f64::INFINITY {
                    v = v.max(ub);
                }
                vars.add(v)
            })
            .collect_vec();

        // 2. Definition of the objective function
        let mut obj_expr = Expression::from(0.0);
        for (i, &coeff) in problem.get_obj_coeffs().iter().enumerate() {
            if coeff != 0.0 {
                obj_expr += coeff * good_vars[i];
            }
        }

        // 3. Model instanciation based on the direction
        let mut model = match problem.get_direction() {
            OptimizationDirection::Maximize => vars.maximise(obj_expr).using(solver),
            OptimizationDirection::Minimize => vars.minimise(obj_expr).using(solver),
        };

        // 4. Add all the constraints to the model
        for (coef, cmp_op, rhs) in problem.get_constraints().iter() {
            let mut lhs_expr = Expression::from(0.0);
            for (var_idx, &coeff) in coef.iter() {
                lhs_expr += coeff * good_vars[var_idx];
            }

            let constraint_obj = match *cmp_op {
                ComparisonOp::Le => constraint!(lhs_expr <= *rhs),
                ComparisonOp::Ge => constraint!(lhs_expr >= *rhs),
                ComparisonOp::Eq => constraint!(lhs_expr == *rhs),
            };

            model = model.with(constraint_obj);
        }

        Ok(model)
    }

    fn compare_execution_highs(path: &str) {
        let mut logger = Logger::load_from(path).expect("No such file");

        let mut feas_checker = logger
            .problem
            .create_feasibility_checker()
            .expect("Error while creating the feasability checker");

        feas_checker
            .check_feasibility()
            .expect("With no constraint active, the problem should be feasible");

        println!("Number events: {}", logger.stack_event.stack.len());

        for (i, &BoundSetEvent { var, bound, val }) in logger.stack_event.iter().enumerate() {
            logger.problem.set_bound(var, &bound, val);
            let res_set_bound = feas_checker.set_bound(var, &bound, val);
            let res_check_feas = feas_checker.check_feasibility();

            let model = build_good_lp_model(&logger.problem, highs).expect("Error while creating highs instance");

            let res_highs = model.solve();

            // assert_eq!(
            //     res_highs.is_ok(),
            //     res_check_feas.is_ok() && res_set_bound.is_ok(),
            //     "Solutions differ at iteration : {i}"
            // );

            if res_highs.is_ok() != (res_check_feas.is_ok() && res_set_bound.is_ok()) {
                println!("Solutions differ at iteration : {i}");
            }
        }
    }

    #[test]
    fn load_burma14() {
        compare_execution_highs("/home/mseraud/Documents/log/burma14.log");
    }

    #[test]
    fn load_ulysses16() {
        compare_execution_highs("/home/mseraud/Documents/log/ulysses16.log");
    }
}
