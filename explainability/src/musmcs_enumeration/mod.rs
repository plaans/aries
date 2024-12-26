pub mod marco;

use std::collections::BTreeSet;
use std::sync::Arc;

use aries::core::Lit;
use aries::model::{Label, Model};
use itertools::Itertools;

pub struct MusMcsEnumerationConfig {
    pub return_muses: bool,
    pub return_mcses: bool,
}

#[derive(Clone)]
pub struct MusMcsEnumerationResult {
    pub muses: Option<Vec<BTreeSet<Lit>>>,
    pub mcses: Option<Vec<BTreeSet<Lit>>>,
}

// #[derive(Clone)]
// struct SoftConstraintPresences<Lbl: Label> {
//     models: Arc<Vec<Model<Lbl>>>,
//     /// Flat indexing: the presence literal for the soft constraint `i` in model `j` is at index `i * (j + 1)`.
//     mapping: Vec<Lit>, 
//     // TODO??: instead of Lit, use Option<Lit>: a None would mean that that soft constraint is not reified / not considered for that model
// }
// 
// impl<Lbl: Label> SoftConstraintPresences<Lbl> {
//     fn get_reif_lit(&self, soft_constr_idx: usize, model_idx: usize) -> Lit {
//         assert!(model_idx < self.models.len(), "Model index out of bounds");
//         assert!(
//             soft_constr_idx * (model_idx + 1) < self.mapping.len(),
//             "Soft constraint index out of bounds"
//         );
//         self.mapping[soft_constr_idx * (model_idx + 1)]
//     }
// 
//     fn get_reif_lit_only_model(&self, soft_constr_idx: usize) -> Lit {
//         assert!(self.models.len() == 1, "Function assumes only one model is registered");
//         self.get_reif_lit(soft_constr_idx, 0)
//     }
// 
//     fn get_reif_lit_all_models(&self, soft_constr_idx: usize) -> Vec<Lit> {
//         assert!(
//             soft_constr_idx * (self.models.len() + 1) < self.mapping.len(),
//             "Soft constraint index out of bounds"
//         );
//         self.mapping
//             .iter()
//             .skip(soft_constr_idx)
//             .step_by(self.models.len())
//             .cloned()
//             .collect_vec()
//     }
// }
// 