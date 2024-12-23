mod marco;

use std::collections::BTreeSet;
use std::sync::Arc;

use aries::core::Lit;
use aries::model::{Label, Model};
use itertools::Itertools;

pub struct MusMcsEnumerationConfig {
    return_muses: bool,
    return_mcses: bool,
}

#[derive(Clone)]
struct SoftConstraintsReifications<Lbl: Label> {
    models: Arc<Vec<Model<Lbl>>>,
    /// Flat indexing: the literal reifying soft constraint `i` in model `j` is at index `i * (j + 1)`.
    mapping: Vec<Lit>,
}

#[derive(Clone)]
pub struct MusMcsEnumerationResult<Lbl: Label> {
    soft_constrs_reifs: SoftConstraintsReifications<Lbl>,
    muses_reif_lits: Option<Vec<BTreeSet<Lit>>>,
    mcses_reif_lits: Option<Vec<BTreeSet<Lit>>>,
}

impl<Lbl: Label> SoftConstraintsReifications<Lbl> {
    //fn new<Expr: Reifiable<Lbl> + Copy>(
    //    models: Vec<&Model<Lbl>>,
    //    soft_constrs: Vec<Expr>,
    //) -> Self {
    //
    //    let mut models = models.into_iter().cloned().collect_vec();
    //    let mut mapping = Vec::<Lit>::new();
    //
    //    for model in models.iter_mut() {
    //        let reif_lits = soft_constrs.iter().map(|&expr| model.reify(expr)).collect_vec();
    //        mapping.extend(reif_lits);
    //    }
    //    SoftConstraintsReifications { models: Arc::new(models), mapping }
    //}

    fn get_reif_lit(&self, soft_constr_idx: usize, model_idx: usize) -> Lit {
        assert!(model_idx < self.models.len(), "Model index out of bounds");
        assert!(
            soft_constr_idx * (model_idx + 1) < self.mapping.len(),
            "Soft constraint index out of bounds"
        );
        self.mapping[soft_constr_idx * (model_idx + 1)]
    }

    fn get_reif_lit_only_model(&self, soft_constr_idx: usize) -> Lit {
        assert!(self.models.len() == 1, "Function assumes only one model is registered");
        self.get_reif_lit(soft_constr_idx, 0)
    }

    fn get_reif_lit_all_models(&self, soft_constr_idx: usize) -> Vec<Lit> {
        assert!(
            soft_constr_idx * (self.models.len() + 1) < self.mapping.len(),
            "Soft constraint index out of bounds"
        );
        self.mapping
            .iter()
            .skip(soft_constr_idx)
            .step_by(self.models.len())
            .cloned()
            .collect_vec()
    }
}
