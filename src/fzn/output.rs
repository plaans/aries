//! Flatzinc ouput.

use crate::fzn::var::Assignment;
use crate::fzn::Fzn;

/// Message when the problem is unsatisfiable.
pub const UNSAT: &str = "=====UNSATISFIABLE=====";

/// Message when at the end of a solution.
pub const END_OF_SOLUTION: &str = "----------";

/// Message when all solutions are printed.
pub const END_OF_SEARCH: &str = "==========";

/// Create a string representing the output for a solution.
pub fn make_output(result: Option<Vec<Assignment>>) -> String {
    match result {
        Some(assignements) => {
            let mut output = String::new();
            for assignment in assignements {
                if assignment.output() {
                    output += assignment.fzn().as_str();
                    output += "\n";
                }
            }
            output += END_OF_SOLUTION;
            output
        }
        None => UNSAT.to_string(),
    }
}
