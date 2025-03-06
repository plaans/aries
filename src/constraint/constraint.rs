use crate::parvar::ParVar;

pub trait Constraint {
    fn build(args: Vec<ParVar>) -> anyhow::Result<Self> where Self: Sized;
    fn name(&self) -> &'static str;
    fn args(&self) -> Vec<ParVar>;
}