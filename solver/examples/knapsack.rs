mod utils;

use aries_solver::{prelude::*, reasoners::lp::LP_ENABLE};
use clap::Parser;
use std::path::PathBuf;

fn solve(items: &[(IntCst, IntCst)], capacity: IntCst) -> Option<IntCst> {
    let mut model = Model::new();

    // create one decision variable for each item, with value:
    //  -  0 if the of item is absent from the solution
    //  -  1 if it is selected
    let vars: Vec<Var> = items.iter().map(|_| model.new_variable(0, 1)).collect();

    // create linear expressions containing the sum of weight/value for all present items
    let mut total_weight = LinSum::zero();
    let mut total_value = LinSum::zero();
    for ((weight, value), var) in items.iter().copied().zip(vars.iter().copied()) {
        total_weight += var * weight;
        total_value += var * value;
    }

    let total_value = total_value.reify([], &mut model); // TODO: reify on model

    model.enforce(total_weight.leq(capacity));

    let mut solver = Solver::new(model);

    if let Some((objective_value, solution)) = solver.maximize(total_value, SearchLimit::None).unwrap() {
        println!("Found objective: {objective_value} (optimal)");
        print!("Selected objects:");
        for (object_id, variable) in vars.iter().enumerate() {
            if solution.eval(*variable).is_some_and(|value| value >= 1) {
                print!(" {object_id}")
            }
        }
        println!();
        Some(objective_value)
    } else {
        println!("No solution");
        None
    }
}

/// A simple 0-1 knapsack solver.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the instance
    file: PathBuf,
    /// If set, the solver will crash if it does not find the given optimum value.
    #[arg(long)]
    expected_value: Option<IntCst>,
}

fn main() {
    // LP relaxation is critical for decent performance on these problems.
    LP_ENABLE.set(true);
    let args = Args::parse();

    let Ok(file_content) = std::fs::read_to_string(&args.file) else {
        panic!("Unable to read input file: {:?}", &args.file)
    };

    let mut parser = crate::utils::Parser::new(&file_content);
    let num_items: usize = parser.pop();
    let capacity: IntCst = parser.pop();
    let mut items = Vec::with_capacity(num_items);
    for _ in 0..num_items {
        // (weight, value) for each item
        let value = parser.pop();
        let weight = parser.pop();
        items.push((weight, value));
    }

    let result = solve(&items, capacity);
    if let Some(optimal) = args.expected_value {
        assert_eq!(result, Some(optimal));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_knapsack_example() {
        let items = vec![(4, 5), (2, 7), (7, 10), (1, 1)];
        assert_eq!(solve(&items, 7), Some(13));

        let items = vec![(4, 5), (2, 7), (7, 10), (2, 1)];
        assert_eq!(solve(&items, 7), Some(12));
    }
}
