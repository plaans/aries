use std::ops::Add;

#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum EqRelation {
    Eq,
    Neq,
}

impl Add for EqRelation {
    type Output = Option<Self>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (EqRelation::Eq, EqRelation::Eq) => Some(EqRelation::Eq),
            (EqRelation::Neq, EqRelation::Eq) => Some(EqRelation::Neq),
            (EqRelation::Eq, EqRelation::Neq) => Some(EqRelation::Neq),
            (EqRelation::Neq, EqRelation::Neq) => None,
        }
    }
}
