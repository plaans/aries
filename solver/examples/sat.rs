use aries_solver::prelude::*;
use itertools::Itertools;

/// Solves a SAT problem in its CNF form
///
/// We consider that the first variable is always 1
/// Example: [[1, 2], [-2, 3, 4]] represents (x1 \/ x2) /\ (not x2 \/ x3 \/ x4)

fn solve_sat(instance: &[Vec<IntCst>]) -> Option<Vec<bool>> {
    let mut model = Model::new();

    let nb_var: u32 = instance.iter().flatten().map(|x| x.abs()).max().unwrap() as u32;

    let variables: Vec<Lit> = (0..nb_var).map(|_| model.new_variable(0, 1).geq(1)).collect();

    for disj_raw in instance {
        let disj_refined: Vec<Lit> = disj_raw
            .iter()
            .map(|&lit| {
                let var = variables[lit.abs() - 1];
                if lit > 0 { var } else { !var }
            })
            .collect();

        model.enforce(or(disj_refined), []);
    }

    // Create the solver and search for a solution
    let mut solver = Solver::new(model);

    match solver.solve(SearchLimit::None) {
        Ok(Some(solution)) => {
            // Extract the value for our boolean variables
            let values: Vec<bool> = variables
                .iter()
                .map(|&q| solution.eval(q).expect("Our variable should have a value"))
                .collect();

            println!("Found a solution for this SAT instance:");
            print_instance(&instance);
            print_solution_values(&values);

            Some(values)
        }
        Ok(None) => {
            println!("This SAT instance is unsatisfiable:");
            print_instance(&instance);
            None
        }
        Err(e) => {
            println!("Solver error: {}", e);
            None
        }
    }
}

fn format_literal(lit: IntCst) -> String {
    if lit > 0 {
        format!("x{}", lit)
    } else {
        format!("not x{}", -lit)
    }
}

fn format_clause(clause: &[IntCst]) -> String {
    clause
        .iter()
        .map(|&lit| format_literal(lit))
        .collect::<Vec<_>>()
        .join(" \\/ ")
}

fn format_formula(formula: &[Vec<IntCst>]) -> String {
    formula
        .iter()
        .map(|clause| format!("({})", format_clause(clause)))
        .collect::<Vec<_>>()
        .join(" /\\ ")
}

fn print_instance(formula: &[Vec<IntCst>]) {
    println!("{}", format_formula(formula));
}

fn print_solution_values(values: &[bool]) {
    for (i, value) in values.iter().enumerate() {
        print!("x{} = {value}, ", i + 1);
    }
    println!();
    println!();
}

fn main() {
    // Solve the following SAT instances
    solve_sat(&vec![vec![1], vec![-2], vec![3]]);

    solve_sat(&vec![vec![1, 2], vec![-1, 2], vec![1, -2], vec![-1, -2, 3], vec![3]]);

    solve_sat(&vec![
        vec![1, 2],
        vec![-1, 2],
        vec![-2, 3],
        vec![2, -3],
        vec![-3, 4],
        vec![3, -4],
        vec![4, 5],
        vec![-4, 5],
        vec![1, -5],
    ]);

    solve_sat(&vec![vec![1], vec![-1]]);
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
    fn test_simple_instance() {
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
    fn test_unsatisfiable_instance() {
        let instance = vec![vec![1], vec![-1]];
        let solution = solve_sat(&instance);

        assert!(solution.is_none());
    }

    fn verify_solution(solution: Option<Vec<bool>>, instance: &[Vec<IntCst>]) {
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
