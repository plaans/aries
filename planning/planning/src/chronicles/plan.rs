use aries::model::lang::Cst;
use num_rational::Rational32;

#[derive(Clone)]
pub struct ActionInstance {
    pub name: String,
    pub params: Vec<Cst>,
    pub start: Rational32,
    pub duration: Rational32,
}
