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
}

/// Wrap the original problem with no constraint activated with its execution stack
///
/// Used to store all the necessary information at once for re-execution
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

    /// Saves the Logger into the file located at path
    pub fn save_to(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);

        postcard::to_io(self, writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use good_lp::{Expression, ProblemVariables, Solver, SolverModel, constraint, highs};
    use itertools::Itertools;
    use minilp::{ComparisonOp, Error, OptimizationDirection};

    use super::*;

    impl StackEvent {
        /// Returns an iterator over the BoundSetEvents contained in the stack
        pub fn iter(&self) -> impl Iterator<Item = &BoundSetEvent> {
            self.stack.iter()
        }
    }

    /// Load a Logger from a file
    impl Logger {
        pub fn load_from(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let bytes = std::fs::read(path)?;

            let logger: Logger = postcard::from_bytes(&bytes)?;

            Ok(logger)
        }
    }

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

    /// Load a Logger from the given path and solve is using minilp, minilp incremental and highs to compare their results
    fn compare_execution_highs(path: &str) {
        let mut logger = Logger::load_from(path).expect("No such file");

        let mut feas_checker = logger
            .problem
            .create_feasibility_checker()
            .expect("Error while creating the feasability checker");

        println!("Number events: {}", logger.stack_event.stack.len());

        for (i, &BoundSetEvent { var, bound, val }) in logger.stack_event.iter().enumerate() {
            logger.problem.set_bound(var, &bound, val);

            let res_set_bound_incr = feas_checker.set_bound(var, &bound, val);
            let mut res_check_feas_incr = feas_checker.check_feasibility();

            if res_set_bound_incr.is_ok() {
                res_check_feas_incr = feas_checker.check_feasibility();
            }

            let model = build_good_lp_model(&logger.problem, highs).expect("Error while creating highs instance");

            let res_solve_reload = logger.problem.solve();

            let res_highs = model.solve();

            if res_highs.is_ok() != res_solve_reload.is_ok() {
                println!("Reload: solutions differ at iteration : {}", i + 1);
            }

            if res_highs.is_ok() != (res_set_bound_incr.is_ok() && res_check_feas_incr.is_ok()) {
                println!("Incremental: solutions differ at iteration : {}", i + 1);

                if let Err(Error::InfeasibleWithCertificate(cert)) = res_check_feas_incr
                    && !logger.problem.is_certificate_valid(&cert)
                {
                    println!("But certificate is invalid")
                }

                if let Err(Error::InfeasibleWithCertificate(cert)) = res_set_bound_incr
                    && !logger.problem.is_certificate_valid(&cert)
                {
                    println!("But certificate is invalid")
                }
            }
        }
    }

    #[test]
    fn load_burma14() {
        println!("Burma 14:");
        compare_execution_highs("/home/mseraud/Documents/log/burma14.log");
    }

    #[ignore]
    #[test]
    fn load_ulysses16() {
        println!("Ulysses 16:");
        compare_execution_highs("/home/mseraud/Documents/log/ulysses16.log");
    }
}
