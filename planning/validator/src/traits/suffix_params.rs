use anyhow::Result;

/// Represents a struct containing parameters which can be suffixed.
pub trait SuffixParams {
    fn suffix_params_with(&mut self, suffix: &str) -> Result<()>;
}
