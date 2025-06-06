use aries::model::lang::{Cst, Rational};
use std::fmt::{Debug, Formatter};

#[derive(Clone)]
pub struct ActionInstance {
    pub name: String,
    pub params: Vec<Cst>,
    pub start: Rational,
    pub duration: Rational,
}

impl Debug for ActionInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}({}) [{}]",
            self.start,
            self.name,
            self.params
                .iter()
                .map(|p| format!("{:?}", p))
                .collect::<Vec<_>>()
                .join(", "),
            self.duration
        )
    }
}
