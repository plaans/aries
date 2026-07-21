pub mod states;
pub mod instance;
pub mod json_instance;
pub mod actions;

pub type JigId = usize;
pub type JigTypeId = usize;
pub type BelugaId = usize;
pub type TrailerId = usize;
pub type HangarId = usize;
pub type ProdLineId = usize;
pub type RackId = usize;

#[derive(Debug)]

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Side {
    Beluga,
    Factory,
}
