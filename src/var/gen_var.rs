use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use crate::domain::BoolDomain;
use crate::domain::IntDomain;
use crate::traits::Flatzinc;
use crate::traits::Name;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(PartialEq, Eq, Debug)]
pub struct GenVar<D> {
    id: usize,
    domain: D,
    name: String,
}

impl<D> GenVar<D> {
    pub(crate) fn new(domain: D, name: String) -> Self {
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
    fn name(&self) -> &String {
        &self.name
    }
}

pub type VarBool = GenVar<BoolDomain>;
pub type VarInt = GenVar<IntDomain>;

impl Flatzinc for VarBool {
    fn fzn(&self) -> String {
        format!("var bool: {};\n", self.name)
    }
}

// TODO: add domain
impl Flatzinc for VarInt {
    fn fzn(&self) -> String {
        format!("var int: {};\n", self.name)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::IntRange;

    use super::*;

    #[test]
    fn id() -> anyhow::Result<()> {
        let x = VarBool::new(BoolDomain, "x".to_string());
        let y = VarBool::new(BoolDomain, "y".to_string());
        let z = VarInt::new(IntRange::new(1, 2)?.into(), "y".to_string());

        assert_ne!(x.id(), y.id());
        assert_ne!(y.id(), z.id());
        assert_ne!(x.id(), z.id());

        Ok(())
    }
}
