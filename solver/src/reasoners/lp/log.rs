use std::fs::File;
use std::io::BufWriter;

use serde::{Deserialize, Serialize};

use crate::core::LongCst;
use minilp::{Bound, Problem, Variable};

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct BoundSetEvent {
    var: Variable,
    bound: Bound,
    val: LongCst,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StackEvent {
    stack: Vec<BoundSetEvent>,
}

impl StackEvent {
    pub fn new() -> Self {
        StackEvent { stack: Vec::new() }
    }

    pub fn push_event(&mut self, var: Variable, bound: Bound, val: LongCst) {
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
    use crate::reasoners::lp::log::Logger;

    #[test]
    fn load_default() {
        let logger = Logger::load_from("/home/mseraud/Documents/log/default.log").expect("No default file");

        println!("Problem: {:?}", logger.problem);
    }
}
