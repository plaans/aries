use aries_solver::prelude::*;

/// Solves the given sudoku
///
/// The input grid must contains 0 for empty cells and number between 1 and 9 for the fixed one

fn solve_sudoku(initial_grid: &[Vec<usize>]) -> Option<Vec<Vec<usize>>> {
    let mut model = Model::new();

    let variables_grid: Vec<Vec<Var>> = (0..9)
        .map(|_| (0..9).map(|_| model.new_variable(1, 9)).collect())
        .collect();

    for i in 0..9usize {
        for j in 0..9usize {
            // We force fixed cells to have the correct value
            if initial_grid[i][j] != 0 {
                model.enforce(eq(variables_grid[i][j], initial_grid[i][j] as IntCst), []);
            }
        }
    }

    for i in 0..9usize {
        for j in 0..9usize {
            for k in j + 1..9usize {
                // We force cells on the same line to be different
                model.enforce(neq(variables_grid[i][j], variables_grid[i][k]), []);

                // We force cells on the same column to be different
                model.enforce(neq(variables_grid[j][i], variables_grid[k][i]), []);
            }

            // We determine the position of the top left cell of the region we are in
            let row_tl_cell = 3 * (i / 3);
            let col_tl_cell = 3 * (j / 3);

            let lower_bound_region = (i - row_tl_cell) * 3 + (j - col_tl_cell) + 1;

            for k in lower_bound_region..9 {
                //We force cells on the same region to be different
                model.enforce(
                    neq(
                        variables_grid[i][j],
                        variables_grid[row_tl_cell + k / 3][col_tl_cell + k % 3],
                    ),
                    [],
                );
            }
        }
    }

    // Create the solver and search for a solution
    let mut solver = Solver::new(model);

    println!("Initial grid:");
    print_grid(&initial_grid);
    println!("");

    match solver.solve(SearchLimit::None) {
        Ok(Some(solution)) => {
            // Extract the solved grid
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
            println!("No solution found for this sudoku");
            None
        }
        Err(e) => {
            println!("Solver error: {}", e);
            None
        }
    }
}

fn print_grid(grid: &[Vec<usize>]) {
    for i in 0..9usize {
        for j in 0..9usize {
            print!("{} ", grid[i][j]);
            if j == 2 || j == 5 {
                print!("| ")
            }
        }
        println!("");
        if i == 2 || i == 5 {
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
