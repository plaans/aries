use aries::prelude::*;

type Model = aries::prelude::Model<String>; // TODO: get rid of type parameter

fn solve(items: &[(IntCst, IntCst)], capacity: IntCst) -> Option<IntCst> {
    let mut model = Model::new();

    // create one decision variable for each item, with value:
    //  -  0 if the of item is absent from the solution
    //  -  1 if it is selected
    let vars: Vec<VarRef> = items.iter().map(|_| model.new_variable(0, 1)).collect();

    // create linear expressions containing the sum of weight/value for all present items
    let mut total_weight = LinSum::zero();
    let mut total_value = LinSum::zero();
    for ((weight, value), var) in items.iter().copied().zip(vars.iter().copied()) {
        total_weight += var * weight;
        total_value += var * value;
    }

    let total_value = total_value.reify([], &mut model); // TODO: reify on model

    model.enforce(total_weight.leq(capacity), []);

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

fn main() {
    // (weight, value) for each item
    let items = vec![(4, 5), (2, 7), (7, 10), (1, 1)];
    assert_eq!(solve(&items, 7), Some(13));
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
