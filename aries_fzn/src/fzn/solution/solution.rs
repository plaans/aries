use crate::aries::Solver;
use crate::fzn::solution::Assignment;
use crate::fzn::Fzn;

/// Message to indicate the problem is unsatisfiable.
pub const UNSAT: &str = "=====UNSATISFIABLE=====";

/// Message to indicate the end of a solution.
pub const END_OF_SOLUTION: &str = "----------";

/// Message when to indicate all solutions have been found.
pub const END_OF_SEARCH: &str = "==========";

/// Flatzinc solution.
///
/// ```flatzinc
/// x = 4;
/// y = 1;
/// b = true;
/// ----------
/// ```
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Solution {
    assignments: Vec<Assignment>,
}

impl Solution {
    pub fn new(assignments: Vec<Assignment>) -> Self {
        Self { assignments }
    }

    pub fn assignments(&self) -> &Vec<Assignment> {
        &self.assignments
    }
}

impl Fzn for Solution {
    fn fzn(&self) -> String {
        let mut s = String::new();
        for assignment in &self.assignments {
            s += assignment.fzn().as_str();
            s += "\n"
        }
        s += END_OF_SOLUTION;
        s += "\n";
        s
    }
}

impl Fzn for Option<Solution> {
    fn fzn(&self) -> String {
        match self {
            Some(solution) => solution.fzn(),
            None => UNSAT.to_string(),
        }
    }
}

/// The callback is called once per textual info (solution or unsat message).
pub fn make_output_flow<F>(solver: &Solver, mut f: F) -> anyhow::Result<()>
where
    F: FnMut(String),
{
    let g = |solution: Solution| {
        f(solution.fzn());
    };
    let sat = solver.solve_with(g)?;

    if !sat {
        f(UNSAT.to_string() + "\n");
    } else {
        f(END_OF_SEARCH.to_string() + "\n");
    }
    Ok(())
}
