mod simple_marco;

use super::{MusMcsEnumerationConfig, MusMcsEnumerationResult};

use aries::core::Lit;
use aries::model::{Label, Model};
use aries::reif::Reifiable;
use aries::solver::Solver;
use std::collections::BTreeSet;

trait Marco<Lbl: Label> {
    fn new<Expr: Reifiable<Lbl> + Copy>(
        model: Model<Lbl>,
        soft_constrs: Vec<Expr>,
        config: MusMcsEnumerationConfig,
    ) -> Self;

    fn reset_result(&mut self);

    fn clone_result(&self) -> MusMcsEnumerationResult<Lbl>;

    fn get_result(&self) -> &MusMcsEnumerationResult<Lbl>;

    fn find_unexplored_seed(&mut self) -> bool;

    fn check_seed_sat(&mut self) -> bool;

    fn do_case_seed_sat(&mut self);

    fn do_case_seed_unsat(&mut self);

    fn run(&mut self) -> MusMcsEnumerationResult<Lbl> {
        self.reset_result();
        while self.find_unexplored_seed() {
            if self.check_seed_sat() {
                self.do_case_seed_sat();
            } else {
                self.do_case_seed_unsat();
            }
        }
        self.clone_result()
    }
}

trait MapSolver<Lbl: Label> {
    fn find_unexplored_seed(&mut self) -> Option<BTreeSet<Lit>>;

    fn block_down(&mut self, frompoint: &BTreeSet<Lit>);

    fn block_up(&mut self, frompoint: &BTreeSet<Lit>);

    fn get_internal_solver(&mut self) -> &mut Solver<Lbl>;
}

trait SubsetSolver<Lbl: Label> {
    fn check_seed_sat(&mut self, seed: &BTreeSet<Lit>) -> bool;

    fn grow(&mut self, seed: &BTreeSet<Lit>) -> (BTreeSet<Lit>, Option<BTreeSet<Lit>>);

    fn shrink(&mut self, seed: &BTreeSet<Lit>) -> BTreeSet<Lit>;

    fn get_internal_solver(&mut self) -> &mut Solver<Lbl>;
}
