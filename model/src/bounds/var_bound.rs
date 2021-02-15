use crate::lang::VarRef;

/// Represents the upped or the lower bound of a particular variable.
/// The type has dense integer values and can by used an index in an array.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VarBound(u32);

impl VarBound {
    pub fn new_raw(id: u32) -> Self {
        VarBound(id)
    }

    pub fn ub(v: VarRef) -> Self {
        VarBound(u32::from(v) << 1)
    }

    pub fn lb(v: VarRef) -> Self {
        VarBound((u32::from(v) << 1) + 1)
    }

    pub fn variable(self) -> VarRef {
        VarRef::from(self.0 >> 1)
    }
}

impl From<VarBound> for u32 {
    fn from(vb: VarBound) -> Self {
        vb.0 as u32
    }
}

impl From<u32> for VarBound {
    fn from(u: u32) -> Self {
        VarBound::new_raw(u as u32)
    }
}

impl From<VarBound> for usize {
    fn from(vb: VarBound) -> Self {
        vb.0 as usize
    }
}

impl From<usize> for VarBound {
    fn from(u: usize) -> Self {
        VarBound::new_raw(u as u32)
    }
}
