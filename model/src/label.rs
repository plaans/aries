use std::fmt::Debug;

/// Trait requiring the minimum capabilities for a type to serve as the label of variables.
pub trait Label: Debug + Clone + Send + Sync + 'static {
    /// Return the representation of the `0` constant.
    fn zero() -> Self;

    /// TODO: accept pointer to reified expression
    fn reified() -> Self;
}

impl Label for String {
    fn zero() -> Self {
        "ZERO".to_string()
    }

    fn reified() -> Self {
        "REIFIED".to_string()
    }
}
impl Label for &'static str {
    fn zero() -> Self {
        "ZERO"
    }

    fn reified() -> Self {
        "REIFIED"
    }
}
