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

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;

    use aries::model::lang::expr::lt;

    use super::{Marco, MusMcsEnumerationConfig};

    type Model = aries::model::Model<&'static str>;
    type SimpleMarco = super::simple_marco::SimpleMarco<&'static str>;

    #[test]
    fn test_simple_marco_simple() {
        let mut model: Model = Model::new();
        let x0 = model.new_ivar(0, 10, "x0");
        let x1 = model.new_ivar(0, 10, "x1");
        let x2 = model.new_ivar(0, 10, "x2");
        let soft_constrs = vec![lt(x0, x1), lt(x1, x2), lt(x2, x0), lt(x0, x2)];

        let mut simple_marco = SimpleMarco::new(
            model,
            soft_constrs,
            MusMcsEnumerationConfig {
                return_muses: true,
                return_mcses: true,
            },
        );
        let res = simple_marco.run();

        let computed_muses = res.muses_reif_lits.unwrap().into_iter().collect::<BTreeSet<_>>();
        let computed_mcses = res.mcses_reif_lits.unwrap().into_iter().collect::<BTreeSet<_>>();

        let expected_muses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![
                res.soft_constrs_reifs.get_reif_lit(0, 0),
                res.soft_constrs_reifs.get_reif_lit(1, 0),
                res.soft_constrs_reifs.get_reif_lit(2, 0),
            ]),
            BTreeSet::from_iter(vec![
                res.soft_constrs_reifs.get_reif_lit(2, 0),
                res.soft_constrs_reifs.get_reif_lit(3, 0),
            ]),
        ]);

        let expected_mcses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![res.soft_constrs_reifs.get_reif_lit(2, 0)]),
            BTreeSet::from_iter(vec![
                res.soft_constrs_reifs.get_reif_lit(0, 0),
                res.soft_constrs_reifs.get_reif_lit(3, 0),
            ]),
            BTreeSet::from_iter(vec![
                res.soft_constrs_reifs.get_reif_lit(1, 0),
                res.soft_constrs_reifs.get_reif_lit(3, 0),
            ]),
        ]);

        assert_eq!(computed_muses, expected_muses);
        assert_eq!(computed_mcses, expected_mcses);
    }
}
