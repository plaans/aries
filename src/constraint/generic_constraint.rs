use crate::parvar::ParVar;

pub trait Constraint {
    fn create(name: &'static str, args: Vec<ParVar>) -> anyhow::Result<Self> where Self: Sized;
    fn name(&self) -> &'static str;
    fn args(&self) -> impl Iterator<Item = ParVar>;
}