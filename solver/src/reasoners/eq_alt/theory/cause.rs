use crate::reasoners::eq_alt::constraints::ConstraintId;

/// The cause of updates made to the model by the eq propagator
///
/// A.K.A the type of propagation made by eq
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum ModelUpdateCause {
    /// Indicates that a propagator was deactivated due to it creating a cycle with relation Neq.
    /// Independant of presence values.
    /// e.g. a -=> b && b -!=> a
    NeqCycle(ConstraintId),
    // DomUpper,
    // DomLower,
    /// Indicates that a bound update was made due to a Neq path being found
    /// e.g. 1 -=> a && a -!=> b && 0 <= b <= 1 implies b < 1
    DomNeq,
    /// Indicates that a bound update was made due to an Eq path being found
    /// e.g. 1 -=> a && a -=> b implies 1 <= b <= 1
    DomEq,
}

impl From<ModelUpdateCause> for u32 {
    #[allow(clippy::identity_op)]
    fn from(value: ModelUpdateCause) -> Self {
        use ModelUpdateCause::*;
        match value {
            NeqCycle(p) => 0u32 + (u32::from(p) << 1),
            DomNeq => 1u32 + (0u32 << 1),
            DomEq => 1u32 + (1u32 << 1),
        }
    }
}

impl From<u32> for ModelUpdateCause {
    fn from(value: u32) -> Self {
        use ModelUpdateCause::*;
        let kind = value & 0x1;
        let payload = value >> 1;
        match kind {
            0 => NeqCycle(ConstraintId::from(payload)),
            1 => match payload {
                0 => DomNeq,
                1 => DomEq,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
}
