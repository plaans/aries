use aries_solver::prelude::*;

/// Solves the N-Queens problem for a given board size.
///
/// The goal is to place N queens on an NxN chessboard such that no two queens
/// attack each other (i.e., no two queens share the same row, column, or diagonal).
fn solve_nqueens(n: usize) -> Option<Vec<usize>> {
    let mut model = Model::new();

    // Create one variable per row, representing the column position of the queen in that row.
    // Each variable has domain [0, n-1], whose value indicates the column in which the queen is.
    let queens: Vec<Var> = (0..n).map(|_| model.new_variable(0, (n - 1) as IntCst)).collect();

    // For any pair of queens, enforce that they are not on the same column, or diagonal.
    for i in 0..n {
        for j in (i + 1)..n {
            // Different column: queens[i] != queens[j] for all i < j
            model.enforce(neq(queens[i], queens[j]), []);

            // Different "/" diagonal: queens[i] - i != queens[j] - j
            model.enforce(neq(queens[i] - i, queens[j] - j), []);

            // Different "\" diagonal: queens[i] + i != queens[j] + j
            model.enforce(neq(queens[i] + i, queens[j] + j), []);
        }
    }

    // Create the solver and search for a solution
    let mut solver = Solver::new(model);

    match solver.solve(SearchLimit::None) {
        Ok(Some(solution)) => {
            // Extract the column position for each queen
            let positions: Vec<usize> = queens
                .iter()
                .map(|&q| solution.eval(q).expect("Queen variable should have a value") as usize)
                .collect();

            println!("Found solution for {}-Queens:", n);
            print_board(n, &positions);

            Some(positions)
        }
        Ok(None) => {
            println!("No solution found for {}-Queens", n);
            None
        }
        Err(e) => {
            println!("Solver error: {}", e);
            None
        }
    }
}

/// Pretty-print the board configuration
fn print_board(n: usize, positions: &[usize]) {
    for (row, &col) in positions.iter().enumerate() {
        for c in 0..n {
            if c == col {
                print!("Q ");
            } else {
                print!(". ");
            }
        }
        println!(" (row {}, col {})", row, col);
    }
    println!();
}

fn main() {
    // Solve the classic 8-Queens problem
    solve_nqueens(8);

    // Also test with a smaller board
    solve_nqueens(4);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_nqueens_4() {
        let solution = solve_nqueens(4);
        assert!(solution.is_some(), "4-Queens should have a solution");

        // Verify the solution is valid
        if let Some(positions) = solution {
            assert_eq!(positions.len(), 4);
            verify_solution(&positions);
        }
    }

    #[test]
    fn test_nqueens_8() {
        let solution = solve_nqueens(8);
        assert!(solution.is_some(), "8-Queens should have a solution");

        if let Some(positions) = solution {
            assert_eq!(positions.len(), 8);
            verify_solution(&positions);
        }
    }

    /// Verify that a solution is valid (no two queens attack each other)
    fn verify_solution(positions: &[usize]) {
        let n = positions.len();

        for i in 0..n {
            for j in (i + 1)..n {
                // Check columns are different
                assert_ne!(
                    positions[i], positions[j],
                    "Queens at rows {} and {} are in the same column",
                    i, j
                );

                // Check diagonals are different
                assert_ne!(
                    positions[i] as isize - i as isize,
                    positions[j] as isize - j as isize,
                    "Queens at rows {} and {} are on the same / diagonal",
                    i,
                    j
                );

                assert_ne!(
                    positions[i] + i,
                    positions[j] + j,
                    "Queens at rows {} and {} are on the same \\ diagonal",
                    i,
                    j
                );
            }
        }
    }
}
