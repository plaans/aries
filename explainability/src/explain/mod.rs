mod why;
mod presupposition;
mod explanation;

use aries::core::Lit;
use aries::model::Label;
use aries::reif::Reifiable;
use explanation::Explanation;
use presupposition::UnmetPresupposition;

pub type Situation = Vec<Lit>;
pub type Query = Vec<Lit>;

pub type Answer<Lbl> = Result<Explanation<Lbl>, UnmetPresupposition<Lbl>>;

pub trait Question<Lbl: Label, Expr: Reifiable<Lbl>> {
    fn try_answer(&mut self) -> Answer<Lbl> {
        match self.check_presuppositions() {
            Err(unmet_presupposition) => Err(unmet_presupposition),
            Ok(()) => Ok(self.compute_explanation()),
        }
    }

    fn check_presuppositions(&mut self) -> Result<(), UnmetPresupposition<Lbl>>;

    fn compute_explanation(&mut self) -> Explanation<Lbl>;
}
