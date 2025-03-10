use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use crate::domain::BoolDomain;
use crate::domain::IntDomain;
use crate::traits::Name;

#[derive(PartialEq, Eq, Debug)]
pub struct GenVar<D> {
    id: usize,
    domain: D,
    name: Option<String>,
}

impl<D> GenVar<D> {
    
    pub(crate) fn new(domain: D, name: Option<String>) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self { id, name, domain }
    }

    pub fn id(&self) -> &usize {
        &self.id
    }

    pub fn domain(&self) -> &D {
        &self.domain
    }
}

impl<D> Name for GenVar<D> {
    fn name(&self) -> &Option<String> {
        &self.name
    }
}

pub type VarBool = GenVar<BoolDomain>;
pub type VarInt = GenVar<IntDomain>;