use crate::model::Label;
use crate::solver::search::beta::restart::Restart;

#[derive(Clone, Debug)]
pub struct Never;

impl<Lbl: Label> Restart<Lbl> for Never {
    fn restart(&mut self) -> bool {
        false
    }
}
