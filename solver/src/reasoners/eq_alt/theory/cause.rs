use crate::reasoners::eq_alt::constraints::ConstraintId;

/// The cause of updates made to the model by the eq propagator
///
/// A.K.A the type of propagation made by eq
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum ModelUpdateCause {
    /// Indicates that a propagator was deactivated due to it creating a cycle with relation Neq.
    /// Independant of presence values.
    /// e.g. if a -=-> b && b -=-> c && l => c -!=-> a, we infer !l
    NeqCycle(ConstraintId),
    /// Indicates that a constraint was deactivated due to variable bounds.
    /// e.g. if lb(a) > ub(b) && l => a == b, we infer !l
    ///
    /// However, this propagation cannot be explained by constraint bounds alone.
    /// e.g. if dom(a) = {1}, dom(b) = {1, 2}, dom(c) = {1}, l => a -!=-> b && b -==-> c,
    /// we can infer !l despite all bounds being propagated and dom(a) and dom(b) being compatible
    EdgeDeactivation(ConstraintId, bool),
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
            NeqCycle(id) => 0u32 + (u32::from(id) << 2),
            EdgeDeactivation(id, fwd) => 1u32 + (u32::from(fwd) << 2) + (u32::from(id) << 3),
            DomNeq => 2u32 + (0u32 << 2),
            DomEq => 2u32 + (1u32 << 2),
        }
    }
}

impl From<u32> for ModelUpdateCause {
    fn from(value: u32) -> Self {
        use ModelUpdateCause::*;
        let kind = value & 0x3;
        let payload = value >> 2;
        match kind {
            0 => NeqCycle(ConstraintId::from(payload)),
            1 => EdgeDeactivation(ConstraintId::from(payload >> 1), payload & 0x1 > 0),
            2 => match payload {
                0 => DomNeq,
                1 => DomEq,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
}
