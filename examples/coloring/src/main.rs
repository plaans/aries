use std::{env, path::Path};

use aries::{
    model::Model,
    solver::{
        Solver,
        search::activity::{ActivityBrancher, Heuristic},
    },
};
use encode::Encoding;
use parse::Problem;

mod encode;
mod parse;

pub struct LiteralFavoringHeuristic;

impl<L> Heuristic<L> for LiteralFavoringHeuristic {
    fn decision_stage(&self, var: aries::core::VarRef, _: Option<&L>, model: &Model<L>) -> u8 {
        if model.state.bounds(var) == (0, 1) { 0 } else { 1 }
    }
}

fn main() {
    let mut args = env::args();
    args.next().unwrap();
    let path = args.next().unwrap();
    let problem = Problem::from_file(Path::new(&path));
    let mut model = Model::new();
    let encoding = Encoding::new(&problem, &mut model);
    let mut solver = Solver::new(model);
    solver.set_brancher(ActivityBrancher::new_with_heuristic(LiteralFavoringHeuristic {}));
    let res = solver.minimize_with_callback(encoding.n_colors, |n, _| println!("Found solution {n}"));
    if let Ok(Some((n_cols, _))) = res {
        solver.print_stats();
        println!("=========================");
        println!("Found solution: {} colors", n_cols);
        println!("=========================");
    } else {
        panic!();
    }
}
