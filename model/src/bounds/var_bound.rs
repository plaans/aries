/// Represents the upped or the lower bound of a particular variable.
/// The type has dense integer values and can by used an index in an array.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VarBound(u32);

impl VarBound {
    pub fn new(id: u32) -> Self {
        VarBound(id)
    }
}

impl From<VarBound> for u32 {
    fn from(vb: VarBound) -> Self {
        vb.0 as u32
    }
}

impl From<u32> for VarBound {
    fn from(u: u32) -> Self {
        VarBound::new(u as u32)
    }
}

impl From<VarBound> for usize {
    fn from(vb: VarBound) -> Self {
        vb.0 as usize
    }
}

impl From<usize> for VarBound {
    fn from(u: usize) -> Self {
        VarBound::new(u as u32)
    }
}
