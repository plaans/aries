use crate::Lit;
use crate::WriterId;

/// Cause of an event that originates from outside of the solver.
/// It can be either an arbitrary decision or the result of an inference from a module not
/// in the core model.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Cause {
    Decision,
    /// The event is due to an inference.
    /// A WriterID identifies the module that made the inference.
    /// 64 bits are available for the writer to store additional metadata of the inference made.
    /// These can for instance be used to indicate the particular constraint that caused the change.
    /// When asked to explain an inference, both fields are made available to the explainer.
    Inference(InferenceCause),
}

impl Cause {
    pub fn inference(writer: WriterId, payload: impl Into<u32>) -> Self {
        Cause::Inference(InferenceCause {
            writer,
            payload: payload.into(),
        })
    }
}

impl From<Cause> for DirectOrigin {
    fn from(c: Cause) -> Self {
        match c {
            Cause::Decision => DirectOrigin::Decision,
            Cause::Inference(i) => DirectOrigin::ExternalInference(i),
        }
    }
}

impl From<Cause> for Origin {
    fn from(c: Cause) -> Self {
        match c {
            Cause::Decision => Origin::Direct(DirectOrigin::Decision),
            Cause::Inference(i) => Origin::Direct(DirectOrigin::ExternalInference(i)),
        }
    }
}

/// Represent the origin of an event caused by an inference.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct InferenceCause {
    /// A WriterID identifies the module that made the inference.
    pub writer: WriterId,
    /// 64 bits are available for the writer to store additional metadata of the inference made.
    /// These can for instance be used to indicate the particular constraint that caused the change.
    /// When asked to explain an inference, both fields are made available to the explainer.
    pub payload: u32,
}

/// Origin of an event which can be either internal or external to the core model.
///
/// In a model (i.e. the structure containing the domains of variable), each domain update is associated with an origin.
/// This origin can the be used to provide explanation of an update.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Origin {
    Direct(DirectOrigin),
    /// An attempt to set the literal `l` (first field) to true was impossible to achieve as it would have caused
    /// an empty domain.
    /// Thus, it was propagated to make the variable of `l` absent.
    ///
    /// The second field represents the cause of enforcing `l`.
    PresenceOfEmptyDomain(Lit, DirectOrigin),
}
impl Origin {
    pub const DECISION: Origin = Origin::Direct(DirectOrigin::Decision);

    pub const fn implication_propagation(lit: Lit) -> Origin {
        Origin::Direct(DirectOrigin::ImplicationPropagation(lit))
    }

    pub fn as_external_inference(self) -> Option<InferenceCause> {
        match self {
            Origin::Direct(DirectOrigin::ExternalInference(cause)) => Some(cause),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DirectOrigin {
    Decision,
    /// The event is due to an inference.
    /// A WriterID identifies the module that made the inference.
    /// 64 bits are available for the writer to store additional metadata of the inference made.
    /// These can for instance be used to indicate the particular constraint that caused the change.
    /// When asked to explain an inference, both fields are made available to the explainer.
    ExternalInference(InferenceCause),
    /// The given literal triggered an implication propagation.
    ImplicationPropagation(Lit),
}
