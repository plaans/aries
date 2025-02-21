use crate::types::Id;

pub trait Identifiable {
    /// Return the object id.
    fn id(&self) -> &Id;
}