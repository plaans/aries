use crate::traits::Flatzinc;
use crate::var::Assignment;

const UNSAT: &str = "=====UNSATISFIABLE=====";
const END_OF_SOLUTION: &str = "----------";
const END_OF_SEARCH: &str = "==========";

pub fn make_output(result: Option<Vec<Assignment>>) -> String {
    match result {
        Some(assignements) => {
            let mut output = String::new();
            for assignment in assignements {
                output += assignment.fzn().as_str();
                output += "\n";
            }
            output += END_OF_SOLUTION;
            output
        }
        None => UNSAT.to_string(),
    }
}
