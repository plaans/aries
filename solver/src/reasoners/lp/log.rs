use std::fs::File;
use std::io::BufWriter;

use serde::{Deserialize, Serialize};

use minilp::{Bound, Problem, Variable};

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct BoundSetEvent {
    var: Variable,
    bound: Bound,
    val: f64,
}

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

    use minilp::Error;

    use super::*;

    fn test_execution(path: &str) {
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

            if let Err(Error::InfeasibleWithCertificate(cert)) = res_set_bound
                && !logger.problem.is_certificate_valid(&cert)
            {
                println!("Incremental: unvalid certificate at iteration: {i}");
            }

            if let Err(Error::InfeasibleWithCertificate(cert)) = res_check_feas
                && !logger.problem.is_certificate_valid(&cert)
            {
                println!("Incremental: unvalid certificate at iteration: {i}");
            }

            let res_solve = logger.problem.solve();

            if let Err(Error::InfeasibleWithCertificate(cert)) = res_solve
                && !logger.problem.is_certificate_valid(&cert)
            {
                println!("Full reload: unvalid certificate at iteration: {i}");
            }
        }
    }

    #[test]
    fn load_burma14() {
        test_execution("/home/mseraud/Documents/log/burma14.log");
    }

    #[test]
    fn load_ulysses16() {
        test_execution("/home/mseraud/Documents/log/ulysses16.log");
    }
}
