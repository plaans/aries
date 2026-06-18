#![allow(clippy::needless_range_loop)]

use aries_solver::prelude::*;

/// Solves the given sudoku
///
/// The input grid must contains 0 for empty cells and number between 1 and 9 for the fixed one
fn solve_sudoku(initial_grid: &[Vec<usize>]) -> Option<Vec<Vec<usize>>> {
    let mut model = Model::new();

    // create one variable with domain [1, 9] for each cell
    let variables_grid: Vec<Vec<Var>> = (0..9)
        .map(|_| (0..9).map(|_| model.new_variable(1, 9)).collect())
        .collect();

    // if the input impose a value for a cell, force the corresponding variable to take this value
    for line in 0..9 {
        for col in 0..9 {
            let cell_value = initial_grid[line][col];
            if cell_value != 0 {
                model.enforce(eq(variables_grid[line][col], cell_value as IntCst));
            }
        }
    }
    // enforce that all variables on a line are different
    for line in 0..9 {
        let vars_on_line: Vec<Var> = (0..9).map(|col| variables_grid[line][col]).collect();
        enforce_all_different(&mut model, vars_on_line);
    }
    // enforce that all variables on a column are different
    for col in 0..9 {
        let vars_on_column: Vec<Var> = (0..9).map(|line| variables_grid[line][col]).collect();
        enforce_all_different(&mut model, vars_on_column);
    }

    // enforce that all variables on a 3x3 square are different
    for square in 0..9 {
        // index of top left element of the square
        let tl_line = square / 3 * 3;
        let tl_col = square % 3 * 3;

        let mut vars_in_square = Vec::new();
        for line in tl_line..(tl_line + 3) {
            for col in tl_col..(tl_col + 3) {
                vars_in_square.push(variables_grid[line][col]);
            }
        }
        enforce_all_different(&mut model, vars_in_square);
    }

    // Create the solver and search for a solution
    let mut solver = Solver::new(model);

    println!("Initial grid:");
    print_grid(initial_grid);
    println!();

    match solver.solve(SearchLimit::None) {
        Ok(Some(solution)) => {
            // Extract the filled-up grid
            let solved_grid: Vec<Vec<usize>> = variables_grid
                .iter()
                .map(|v| {
                    v.iter()
                        .map(|&q| solution.eval(q).expect("All cells should have a value") as usize)
                        .collect()
                })
                .collect();

            println!("Solution:");
            print_grid(&solved_grid);

            Some(solved_grid)
        }
        Ok(None) => {
            println!("No solution for this sudoku");
            None
        }
        Err(_) => {
            unreachable!("Without search limit, should never terminate without a result.");
        }
    }
}

/// Helper function that enforces that all variables are different in the given model.
fn enforce_all_different(model: &mut Model, vars: Vec<Var>) {
    for (i, x) in vars.iter().enumerate() {
        for y in &vars[i + 1..] {
            model.enforce(neq(*x, *y));
        }
    }
}

fn print_grid(grid: &[Vec<usize>]) {
    for line in 0..9 {
        for col in 0..9 {
            print!("{} ", grid[line][col]);
            if col == 2 || col == 5 {
                print!("| ")
            }
        }

        println!();

        if line == 2 || line == 5 {
            println!("---------------------")
        }
    }
}

fn main() {
    let grid = vec![
        vec![0, 0, 3, 0, 2, 0, 6, 0, 0],
        vec![9, 0, 0, 3, 0, 5, 0, 0, 1],
        vec![0, 0, 1, 8, 0, 6, 4, 0, 0],
        vec![0, 0, 8, 1, 0, 2, 9, 0, 0],
        vec![7, 0, 0, 0, 0, 0, 0, 0, 8],
        vec![0, 0, 6, 7, 0, 8, 2, 0, 0],
        vec![0, 0, 2, 6, 0, 9, 5, 0, 0],
        vec![8, 0, 0, 2, 0, 3, 0, 0, 9],
        vec![0, 0, 5, 0, 1, 0, 3, 0, 0],
    ];

    solve_sudoku(&grid);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_simple_sudoku() {
        let grid = vec![
            vec![5, 3, 0, 0, 7, 0, 0, 0, 0],
            vec![6, 0, 0, 1, 9, 5, 0, 0, 0],
            vec![0, 9, 8, 0, 0, 0, 0, 6, 0],
            vec![8, 0, 0, 0, 6, 0, 0, 0, 3],
            vec![4, 0, 0, 8, 0, 3, 0, 0, 1],
            vec![7, 0, 0, 0, 2, 0, 0, 0, 6],
            vec![0, 6, 0, 0, 0, 0, 2, 8, 0],
            vec![0, 0, 0, 4, 1, 9, 0, 0, 5],
            vec![0, 0, 0, 0, 8, 0, 0, 7, 9],
        ];

        let solution = solve_sudoku(&grid);
        verify_sudoku_solution(solution, &grid);
    }

    #[test]
    fn test_medium_sudoku() {
        let grid = vec![
            vec![0, 0, 3, 0, 2, 0, 6, 0, 0],
            vec![9, 0, 0, 3, 0, 5, 0, 0, 1],
            vec![0, 0, 1, 8, 0, 6, 4, 0, 0],
            vec![0, 0, 8, 1, 0, 2, 9, 0, 0],
            vec![7, 0, 0, 0, 0, 0, 0, 0, 8],
            vec![0, 0, 6, 7, 0, 8, 2, 0, 0],
            vec![0, 0, 2, 6, 0, 9, 5, 0, 0],
            vec![8, 0, 0, 2, 0, 3, 0, 0, 9],
            vec![0, 0, 5, 0, 1, 0, 3, 0, 0],
        ];

        let solution = solve_sudoku(&grid);
        verify_sudoku_solution(solution, &grid);
    }

    #[test]
    fn test_unsatisfiable_sudoku() {
        let grid = vec![
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
        ];

        let solution = solve_sudoku(&grid);
        assert!(solution.is_none(), "This grid should not admit any solution");
    }

    fn verify_sudoku_solution(solution: Option<Vec<Vec<usize>>>, initial_grid: &[Vec<usize>]) {
        let solved = solution.expect("A solution should have been found");

        assert_eq!(solved.len(), 9);
        for row in &solved {
            assert_eq!(row.len(), 9);
        }

        let mut row_seen = [[false; 9]; 9];
        let mut col_seen = [[false; 9]; 9];
        let mut block_seen = [[false; 9]; 9];

        for i in 0..9 {
            for j in 0..9 {
                let value = solved[i][j];
                assert!((1..=9).contains(&value));

                let idx = value - 1;
                assert!(!row_seen[i][idx], "Duplicate value {} in row {}", value, i);
                assert!(!col_seen[j][idx], "Duplicate value {} in column {}", value, j);

                let block_index = 3 * (i / 3) + (j / 3);
                assert!(
                    !block_seen[block_index][idx],
                    "Duplicate value {} in block {}",
                    value, block_index
                );

                row_seen[i][idx] = true;
                col_seen[j][idx] = true;
                block_seen[block_index][idx] = true;

                if initial_grid[i][j] != 0 {
                    assert_eq!(solved[i][j], initial_grid[i][j], "Fixed cell changed at {} {}", i, j);
                }
            }
        }
    }
}
