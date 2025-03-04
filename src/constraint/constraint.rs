use crate::parvar::ParVar;

pub trait Constraint {
    fn name(&self) -> &'static str;
    fn args(&self) -> Vec<ParVar>;
}