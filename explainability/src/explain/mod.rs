mod explanation;
mod how;
mod presupposition;
mod why;

use std::collections::BTreeSet;

use aries::core::Lit;
use aries::model::Label;
use explanation::Explanation;
use presupposition::PresuppositionStatusCause;

pub type Situation = BTreeSet<Lit>;
pub type Query = BTreeSet<Lit>;
pub type Vocab = BTreeSet<Lit>;

pub type Answer<Lbl> = Result<Explanation<Lbl>, PresuppositionStatusCause>;

pub trait Question<Lbl: Label> {
    fn try_answer(&mut self) -> Answer<Lbl> {
        self.check_presuppositions()?;
        Ok(self.compute_explanation())
    }

    fn check_presuppositions(&mut self) -> Result<(), PresuppositionStatusCause>;

    fn compute_explanation(&mut self) -> Explanation<Lbl>;
}
