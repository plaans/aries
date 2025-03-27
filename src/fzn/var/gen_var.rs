use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use crate::fzn::domain::BoolDomain;
use crate::fzn::domain::IntDomain;
use crate::fzn::Fzn;
use crate::fzn::Name;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(PartialEq, Eq, Debug)]
pub struct GenVar<D> {
    id: usize,
    domain: D,
    name: String,
    output: bool,
}

impl<D> GenVar<D> {
    pub(crate) fn new(domain: D, name: String, output: bool) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            name,
            domain,
            output,
        }
    }

    pub fn id(&self) -> &usize {
        &self.id
    }

    pub fn domain(&self) -> &D {
        &self.domain
    }

    pub fn output(&self) -> bool {
        self.output
    }
}

impl<D> Name for GenVar<D> {
    fn name(&self) -> &String {
        &self.name
    }
}

pub type VarBool = GenVar<BoolDomain>;
pub type VarInt = GenVar<IntDomain>;

impl Fzn for VarBool {
    fn fzn(&self) -> String {
        format!("var bool: {};\n", self.name)
    }
}

// TODO: add domain
impl Fzn for VarInt {
    fn fzn(&self) -> String {
        format!("var int: {};\n", self.name)
    }
}

#[cfg(test)]
mod tests {
    use crate::fzn::domain::IntRange;

    use super::*;

    #[test]
    fn id() -> anyhow::Result<()> {
        let x = VarBool::new(BoolDomain::Both, "x".to_string(), false);
        let y = VarBool::new(BoolDomain::Both, "y".to_string(), false);
        let z =
            VarInt::new(IntRange::new(1, 2)?.into(), "y".to_string(), false);

        assert_ne!(x.id(), y.id());
        assert_ne!(y.id(), z.id());
        assert_ne!(x.id(), z.id());

        Ok(())
    }
}
