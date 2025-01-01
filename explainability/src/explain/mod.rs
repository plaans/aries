mod explanation;
mod how;
mod presupposition;
mod why;

use aries::core::Lit;
use aries::model::Label;
use explanation::Explanation;
use presupposition::UnmetPresupposition;

pub type Situation = Vec<Lit>;
pub type Query = Vec<Lit>;
pub type Vocab = Vec<Lit>;

// pub enum Metric {
//     Maximize(IAtom),
//     Minimize(IAtom),
// }

// ? make a SoftGoals struct / type ? also a Vocab struct / type ? (it could contain metrics info to infer the soft goal / vocab element correpsonding to that metric's value ?)

pub type Answer<Lbl> = Result<Explanation<Lbl>, UnmetPresupposition<Lbl>>;

pub trait Question<Lbl: Label> {
    fn try_answer(&mut self) -> Answer<Lbl> {
        match self.check_presuppositions() {
            Err(unmet_presupposition) => Err(unmet_presupposition),
            Ok(()) => Ok(self.compute_explanation()),
        }
    }

    fn check_presuppositions(&mut self) -> Result<(), UnmetPresupposition<Lbl>>;

    fn compute_explanation(&mut self) -> Explanation<Lbl>;
}
