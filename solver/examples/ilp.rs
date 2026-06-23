use aries_solver::prelude::*;

/// Solves an ILP (Integer Linear Problem) problem in its canonical form:
/// Maximize cx under Ax <= b and x >= 0 where c and b are vectors and A is a matrix
///
/// The following format is used for the input:
///
/// obj_fun is used to define the objective function we want to maximize
/// Example: [1, -2, 4] represents the function x0 - 2*x1 + 4*x2
///
/// constraints contains all the inequalities
/// Example: [[0, 2, 1, 5], [4, 0, 3, 2]] represents 2*x1 + x2 <= 5 and 4*x0 + 3*x2 <= 2
fn solve_ilp(obj_fun: &[IntCst], constraints: &[Vec<IntCst>]) -> Option<(Vec<IntCst>, IntCst)> {
    print_instance(obj_fun, constraints);

    let mut model = Model::new();

    let nb_var = obj_fun.len();

    let variables: Vec<Var> = (0..nb_var).map(|_| model.new_variable(0, INT_CST_MAX)).collect();

    for constraint in constraints {
        let mut lin_sum: LinSum = LinSum::zero();

        // We add all the linear terms of our inequality (a * xn)
        for i in 0..nb_var {
            lin_sum += constraint[i] * variables[i];
        }

        // We finish by adding our constant term that corresponds to the upper bound of our inequality
        let upper_bound = constraint[nb_var];
        let constraint = lin_sum.leq(upper_bound);

        model.enforce(constraint);
    }

    // crate a var that will represent the objective
    let obj_value_var = model.new_variable(INT_CST_MIN, INT_CST_MAX);

    // create a linear sum of the objective function from the input
    let lin_sum_obj: LinSum = obj_fun
        .iter()
        .enumerate()
        .map(|(i, &factor)| factor * variables[i])
        .fold(LinSum::zero(), |acc, term| acc + term);

    // We force our objective variable to correponds to the objective function
    model.enforce(eq(obj_value_var, lin_sum_obj));

    // Create the solver and search for a solution
    let mut solver = Solver::new(model);

    println!("Solving...");

    match solver.maximize(obj_value_var, SearchLimit::None) {
        Ok(Some((obj_value, solution))) => {
            // Extract the value for our xn
            let values: Vec<IntCst> = variables
                .iter()
                .map(|&q| solution.eval(q).expect("Our variable should have a value"))
                .collect();

            println!("=> Optimal (max) value of {obj_value} for this ILP instance:");
            print_solution(&values);
            Some((values, obj_value))
        }
        Ok(None) => {
            println!("=> No solution\n");
            None
        }
        Err(_) => {
            unreachable!("Without a search limit, the solver should always return a value.")
        }
    }
}

fn main() {
    solve_ilp(&[1, 2], &[vec![1, 0, 3], vec![0, 1, 2]]);

    solve_ilp(&[1], &[vec![1, 3], vec![-1, -4]]);

    solve_ilp(
        &[3, 2],
        &[
            vec![1, 1, 4], // x0 + x1 <= 4
            vec![1, 0, 2], // x0 <= 2
            vec![0, 1, 3], // x1 <= 3
        ],
    );

    solve_ilp(
        &[8, 5, 6],
        &[
            vec![2, 1, 1, 7],
            vec![1, 3, 2, 10],
            vec![1, 0, 0, 3],
            vec![0, 1, 0, 3],
            vec![0, 0, 1, 3],
        ],
    );
}

fn print_lin_sum(lin_sum: &[IntCst]) {
    let mut first = true;

    for (i, &factor) in lin_sum.iter().enumerate() {
        if factor == 0 {
            continue;
        }

        if first {
            if factor < 0 {
                print!("-");
            }
        } else {
            print!(" {} ", if factor > 0 { "+" } else { "-" });
        }

        let abs_factor = factor.abs();

        if abs_factor != 1 {
            print!("{abs_factor}*");
        }

        print!("x{i}");

        first = false;
    }

    if first {
        print!("0");
    }
}

fn print_obj_fun(obj_fun: &[IntCst]) {
    print!("Maximize: ");
    print_lin_sum(obj_fun);
    println!();
}

fn print_constraints(constraints: &[Vec<IntCst>], nb_var: usize) {
    println!("Subject to:");
    for constraint in constraints {
        print!("  ");
        print_lin_sum(&constraint[0..nb_var]);
        println!(" <= {}", constraint[nb_var]);
    }
    println!("  for all i, xi in [0, {INT_CST_MAX}]");
}

fn print_instance(obj_fun: &[IntCst], constraints: &[Vec<IntCst>]) {
    println!();
    print_obj_fun(obj_fun);
    print_constraints(constraints, obj_fun.len());
    println!()
}

fn print_solution(values: &[IntCst]) {
    println!("Optimal solution:");

    for (i, value) in values.iter().enumerate() {
        println!("  x{i} = {value}");
    }

    println!();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_unsatisfiable_instance() {
        let obj_fun = vec![1];
        let constraints = vec![vec![1, 3], vec![-1, -4]]; // x0 <= 3 and x0 >= 4 => no solution

        let solution = solve_ilp(&obj_fun, &constraints);

        assert!(solution.is_none(), "There should be no solution");
    }

    #[test]
    fn test_simple_instance() {
        let obj_fun = vec![3, 2];
        let constraints = vec![
            vec![1, 1, 4], // x0 + x1 <= 4
            vec![1, 0, 2], // x0 <= 2
            vec![0, 1, 3], // x1 <= 3
        ];

        let expected_xn = vec![2, 2];
        let expected_obj_value = 10;

        let solution = solve_ilp(&obj_fun, &constraints);

        assert!(solution.is_some(), "A solution should have been found");

        let (xn, obj_value) = solution.unwrap();

        assert_eq!(
            expected_obj_value, obj_value,
            "The obj_value {obj_value} differs from the expected: {expected_obj_value}"
        );

        assert_eq!(
            expected_xn, xn,
            "Values found for the xn {:?} differ from the expected {:?}",
            xn, expected_xn
        );
    }

    #[test]
    fn test_medium_instance() {
        let obj_fun = vec![8, 5, 6];
        let constraints = vec![
            vec![2, 1, 1, 7],
            vec![1, 3, 2, 10],
            vec![1, 0, 0, 3],
            vec![0, 1, 0, 3],
            vec![0, 0, 1, 3],
        ];

        let expected_xn = vec![2, 0, 3];
        let expected_obj_value = 34;

        let solution = solve_ilp(&obj_fun, &constraints);

        assert!(solution.is_some(), "A solution should have been found");

        let (xn, obj_value) = solution.unwrap();

        assert_eq!(
            expected_obj_value, obj_value,
            "The obj_value {obj_value} differs from the expected: {expected_obj_value}"
        );

        assert_eq!(
            expected_xn, xn,
            "Values found for the xn {:?} differ from the expected {:?}",
            xn, expected_xn
        );
    }
}
