use crate::bounds::{BoundValue, VarBound};
use aries_collections::ref_store::RefVec;

pub struct Domains {
    bounds: RefVec<VarBound, BoundValue>,
}
