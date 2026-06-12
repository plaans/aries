use std::vec;

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

fn solve_ilp(obj_fun: &[IntCst], constraints: &[Vec<IntCst>]) -> Option<Vec<IntCst>> {
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
        lin_sum -= constraint[nb_var];

        model.enforce(leq(lin_sum, 0), []);
    }

    let obj_value_var = model.new_variable(INT_CST_MIN, INT_CST_MAX);

    let lin_sum_obj: LinSum = obj_fun
        .iter()
        .enumerate()
        .map(|(i, &factor)| factor * variables[i])
        .fold(LinSum::zero(), |acc, term| acc + term);

    // We force our cost to correponds to the objective function
    model.enforce(eq(obj_value_var, lin_sum_obj), []);

    // Create the solver and search for a solution
    let mut solver = Solver::new(model);

    match solver.maximize(obj_value_var, SearchLimit::None) {
        Ok(Some((obj_value, solution))) => {
            // Extract the value for our boolean variables
            let values: Vec<IntCst> = variables
                .iter()
                .map(|&q| solution.eval(q).expect("Our variable should have a value"))
                .collect();

            println!("Found a maximum value of {obj_value} for this ILP instance:");
            print_instance(obj_fun, constraints);
            print_solution(&values);
            Some(values)
        }
        Ok(None) => {
            println!("This ILP instance has unsatisfiable constraints:");
            print_instance(obj_fun, constraints);
            None
        }
        Err(e) => {
            println!("Solver error: {}", e);
            None
        }
    }
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
    for constraint in constraints {
        print_lin_sum(&constraint[0..nb_var]);

        print!(" <= {}", constraint[nb_var]);

        println!()
    }
}

fn print_instance(obj_fun: &[IntCst], constraints: &[Vec<IntCst>]) {
    print_obj_fun(obj_fun);

    print_constraints(constraints, obj_fun.len());

    println!()
}

fn print_solution(values: &[IntCst]) {
    println!("Solution found:");

    for (i, value) in values.iter().enumerate() {
        print!("x{i} = {value}, ");
    }

    println!();
    println!();
}

fn main() {
    solve_ilp(&vec![1, 2], &vec![vec![1, 0, 3], vec![0, 1, 2]]);

    solve_ilp(&vec![1], &vec![vec![1, 3], vec![-1, -4]]);

    solve_ilp(
        &vec![3, 2],
        &vec![
            vec![1, 1, 4], // x0 + x1 <= 4
            vec![1, 0, 2], // x0 <= 2
            vec![0, 1, 3], // x1 <= 3
        ],
    );

    solve_ilp(
        &vec![8, 5, 6],
        &vec![
            vec![2, 1, 1, 7],
            vec![1, 3, 2, 10],
            vec![1, 0, 0, 3],
            vec![0, 1, 0, 3],
            vec![0, 0, 1, 3],
        ],
    );
}
