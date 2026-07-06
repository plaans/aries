use aries_solver::prelude::*;

/// Solves a SAT problem in its CNF form
///
/// We consider that the first variable is always 1
/// Example: [[1, 2], [-2, 3, 4]] represents (x1 \/ x2) /\ (not x2 \/ x3 \/ x4)
fn solve_sat(instance: &[Vec<i32>]) -> Option<Vec<bool>> {
    print_instance(instance);

    // create a new model that will contain all variables and constraints
    let mut model = Model::new();

    // determine the number of vars needed (largest variable ID in the clauses)
    let nb_var = instance.iter().flatten().map(|literal| literal.abs()).max().unwrap();

    // for each variable in the problem:
    //  - create a new integer variable (`Var`) with domain [0,1]
    //  - transform it into a literal (`Lit`) that acts as a boolean variable that is true iff it has the value 1
    let variables: Vec<Lit> = (0..nb_var).map(|_| model.new_variable(0, 1).geq(1)).collect();

    for disj_raw in instance {
        // create a clause (Disjunction) for this constraint
        let clause = Disjunction::from_iter(disj_raw.iter().map(|&lit| {
            let var = variables[(lit.abs() - 1) as usize];
            if lit > 0 { var } else { !var }
        }));

        // add clause to the model
        model.enforce(clause);
    }

    // Create a solver for the model
    let mut solver = Solver::new(model);

    println!("\nSolving...");

    match solver.solve(SearchLimit::None) {
        Ok(Some(solution)) => {
            // Extract the value for our boolean variables
            let values: Vec<bool> = variables
                .iter()
                .map(|&q| solution.eval(q).expect("Our variable should have a value"))
                .collect();

            println!("=> Instance is SAT");
            print_solution_values(&values);

            Some(values)
        }
        Ok(None) => {
            println!("=> Instance is UNSAT (no solution)");
            None
        }
        Err(_) => {
            unreachable!("Solver should not exit without a solution when no search limit is set");
        }
    }
}

/// Prints a an instance in CNF form
fn print_instance(formula: &[Vec<i32>]) {
    use itertools::Itertools;

    fn format_literal(lit: i32) -> String {
        if lit > 0 {
            format!("x{}", lit)
        } else {
            format!("not x{}", -lit)
        }
    }

    println!("\nInstance:");
    for clause in formula {
        println!("  - {}", clause.iter().map(|&lit| format_literal(lit)).format(" \\/ "))
    }
}

fn print_solution_values(values: &[bool]) {
    println!("\nSolution:");
    for (i, value) in values.iter().enumerate() {
        println!("  x{} = {value}, ", i + 1);
    }
    println!();
}

fn main() {
    // Solve the following SAT instances
    solve_sat(&[vec![1], vec![-2], vec![3]]);

    solve_sat(&[vec![1, 2], vec![-1, 2], vec![1, -2], vec![-1, -2, -3]]);

    solve_sat(&[
        vec![-1, -2],
        vec![-2, 3],
        vec![2, -3],
        vec![-3, 4],
        vec![3, -4],
        vec![4, 5],
        vec![-4, 5],
        vec![1, -5],
    ]);

    solve_sat(&[vec![-1, 2], vec![1, 2], vec![-2, -1], vec![1, -2]]);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_single_disjunction() {
        let instance = vec![vec![1, -2, 3]];
        let solution = solve_sat(&instance);

        verify_solution(solution, &instance);
    }

    #[test]
    fn test_single_conjunction() {
        let instance = vec![vec![1], vec![-2], vec![3]];
        let solution = solve_sat(&instance);

        verify_solution(solution, &instance);
    }

    #[test]
    fn test_2_sat_instance() {
        let instance = vec![
            vec![1, 2],
            vec![-1, 2],
            vec![-2, 3],
            vec![2, -3],
            vec![-3, 4],
            vec![3, -4],
            vec![4, 5],
            vec![-4, 5],
            vec![1, -5],
        ];
        let solution = solve_sat(&instance);

        verify_solution(solution, &instance);
    }

    #[test]
    fn test_3_sat_instance() {
        let instance = vec![
            vec![1, 2, 3],
            vec![1, 2, -3],
            vec![1, -2, 3],
            vec![1, -2, -3],
            vec![-1, 2, 3],
            vec![-1, -2, 3],
            vec![-1, -2, -3],
        ];
        let solution = solve_sat(&instance);

        verify_solution(solution, &instance);
    }

    #[test]
    fn test_unsatisfiable_instance() {
        let instance = vec![vec![1], vec![-1]];
        let solution = solve_sat(&instance);

        assert!(solution.is_none());
    }

    fn verify_solution(solution: Option<Vec<bool>>, instance: &[Vec<i32>]) {
        assert!(solution.is_some(), "A solution should have been found");

        let values = solution.unwrap();

        for disj in instance {
            assert!(
                disj.iter().any(|&lit| {
                    if lit > 0 {
                        values[(lit - 1) as usize]
                    } else {
                        !values[(-lit - 1) as usize]
                    }
                }),
                "The solution doesn't satisfy the disjunction: {:?}",
                disj
            );
        }
    }
}
