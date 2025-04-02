use crate::fzn::var::Assignment;
use crate::fzn::Fzn;

pub const UNSAT: &str = "=====UNSATISFIABLE=====";
pub const END_OF_SOLUTION: &str = "----------";
pub const END_OF_SEARCH: &str = "==========";

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
