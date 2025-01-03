// mod filtered_marco;
pub mod simple_marco;

use crate::musmcs_enumeration::{MusMcsEnumerationConfig, MusMcsEnumerationResult};

use aries::core::Lit;
use aries::model::{Label, Model};
use aries::reif::Reifiable;
use std::collections::BTreeSet;

pub trait Marco<Lbl: Label> {
    fn new_with_soft_constrs_reif_lits(
        model: Model<Lbl>,
        soft_constrs_reif_lits: impl IntoIterator<Item = Lit>,
        config: MusMcsEnumerationConfig,
    ) -> Self;

    fn new_with_soft_constrs<Expr: Reifiable<Lbl>>(
        model: Model<Lbl>,
        soft_constrs: impl IntoIterator<Item = Expr>,
        config: MusMcsEnumerationConfig,
    ) -> Self;

    fn reset_result(&mut self);

    fn clone_result(&self) -> MusMcsEnumerationResult;

    fn get_expr_reif_lit<Expr: Reifiable<Lbl>>(&mut self, soft_constr: Expr) -> Result<Lit, ()>;

    fn find_unexplored_seed(&mut self) -> bool;

    fn check_seed_sat(&mut self) -> bool;

    fn do_case_seed_sat(&mut self);

    fn do_case_seed_unsat(&mut self);

    fn run(&mut self) -> MusMcsEnumerationResult {
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

trait MapSolver {
    fn find_unexplored_seed(&mut self) -> Option<BTreeSet<Lit>>;

    fn block_down(&mut self, frompoint: &BTreeSet<Lit>);

    fn block_up(&mut self, frompoint: &BTreeSet<Lit>);

    // fn get_internal_solver(&mut self) -> &mut Solver<u8>;
}

trait SubsetSolver<Lbl: Label> {
    fn check_seed_sat(&mut self, seed: &BTreeSet<Lit>) -> bool;

    fn grow(&mut self, seed: &BTreeSet<Lit>) -> (BTreeSet<Lit>, Option<BTreeSet<Lit>>);

    fn shrink(&mut self, seed: &BTreeSet<Lit>) -> BTreeSet<Lit>;

    // fn get_internal_solver(&mut self) -> &mut Solver<Lbl>;
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;

    use aries::model::lang::expr::lt;
    use itertools::Itertools;

    use crate::musmcs_enumeration::marco::Marco;
    use crate::musmcs_enumeration::MusMcsEnumerationConfig;

    type Model = aries::model::Model<&'static str>;
    type SimpleMarco = crate::musmcs_enumeration::marco::simple_marco::SimpleMarco<&'static str>;

    #[test]
    fn test_simple_marco_simple() {
        let mut model: Model = Model::new();
        let x0 = model.new_ivar(0, 10, "x0");
        let x1 = model.new_ivar(0, 10, "x1");
        let x2 = model.new_ivar(0, 10, "x2");

        let soft_constrs = vec![lt(x0, x1), lt(x1, x2), lt(x2, x0), lt(x0, x2)];
        let mut simple_marco = SimpleMarco::new_with_soft_constrs(
            model,
            soft_constrs.clone(),
            MusMcsEnumerationConfig {
                return_muses: true,
                return_mcses: true,
            },
        );
        let soft_constrs_reif_lits = soft_constrs
            .into_iter()
            .map(|expr| simple_marco.get_expr_reif_lit(expr))
            .collect_vec();

        let res = simple_marco.run();
        let res_muses = res.muses.unwrap().into_iter().collect::<BTreeSet<_>>();
        let res_mcses = res.mcses.unwrap().into_iter().collect::<BTreeSet<_>>();

        let expected_muses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[0].unwrap(),
                soft_constrs_reif_lits[1].unwrap(),
                soft_constrs_reif_lits[2].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[2].unwrap(),
                soft_constrs_reif_lits[3].unwrap(),
            ]),
        ]);
        let expected_mcses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![soft_constrs_reif_lits[2].unwrap()]),
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[0].unwrap(),
                soft_constrs_reif_lits[3].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[1].unwrap(),
                soft_constrs_reif_lits[3].unwrap(),
            ]),
        ]);

        assert_eq!(res_muses, expected_muses);
        assert_eq!(res_mcses, expected_mcses);
    }
}
