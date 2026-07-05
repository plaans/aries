/// Represents a struct which can have a specific type.
pub trait Typeable {
    fn tpe(&self) -> String;
}
