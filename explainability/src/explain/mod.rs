mod explanation;
mod how;
mod presupposition;
mod why;

use std::collections::BTreeSet;
use std::sync::Arc;

use aries::core::Lit;
use aries::model::{Label, Model};
use aries::reif::Reifiable;
use explanation::Explanation;
use presupposition::PresuppositionStatusCause;

pub type Situation = BTreeSet<Lit>;
pub type Query = BTreeSet<Lit>;

#[derive(Clone)]
pub struct ModelAndVocab<Lbl> {
    pub model: Arc<Model<Lbl>>,
    pub vocab: BTreeSet<Lit>,
}
impl<Lbl: Label> ModelAndVocab<Lbl> {
    pub fn new(
        model: Arc<Model<Lbl>>,
        vocab: impl IntoIterator<Item = Lit>,
    ) -> Self {
        Self {
            model,
            vocab: vocab.into_iter().collect(),
        }
    }

    pub fn model_with_enforced_vocab(&self) -> Model<Lbl> {
        let mut m = (*self.model).clone();
        m.enforce_all(self.vocab.clone(), []);
        m
    }

    pub fn model_with_enforced<Expr: Reifiable<Lbl>>(
        &self,
        to_enforce: impl IntoIterator<Item = Expr>,
    ) -> Model<Lbl> {
        let mut m = (*self.model).clone();
        m.enforce_all(to_enforce, []);
        m
    }
}

pub type Answer<Lbl> = Result<Explanation<Lbl>, PresuppositionStatusCause>;

pub trait Question<Lbl: Label> {
    fn try_answer(&mut self) -> Answer<Lbl> {
        self.check_presuppositions()?;
        Ok(self.compute_explanation())
    }

    fn check_presuppositions(&mut self) -> Result<(), PresuppositionStatusCause>;

    fn compute_explanation(&mut self) -> Explanation<Lbl>;
}
