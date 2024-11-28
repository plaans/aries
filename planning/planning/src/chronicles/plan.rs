use aries::model::lang::{Cst, Rational};

#[derive(Clone)]
pub struct ActionInstance {
    pub name: String,
    pub params: Vec<Cst>,
    pub start: Rational,
    pub duration: Rational,
}
