use std::fmt::Display;

use anyhow::{Ok, Result};

use crate::traits::suffix_params::SuffixParams;

use super::{env::Env, value::Value};

/* ========================================================================== */
/*                                  Parameter                                 */
/* ========================================================================== */

/// Represents a parameter of an action, condition, effect, ...
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    /// The name of the parameter.
    name: String,
    /// The type of the parameter.
    r#type: String,
    /// The value of the parameter when it is instantiated.
    value: Value,
}

impl Parameter {
    pub fn new(name: String, r#type: String, value: Value) -> Self {
        Parameter { name, r#type, value }
    }

    pub fn bound<E>(&self, env: &mut Env<E>) {
        env.bound(self.r#type.clone(), self.name.clone(), self.value.clone());
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

impl Display for Parameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} {}({})", self.r#type, self.name, self.value))
    }
}

impl SuffixParams for Parameter {
    fn suffix_params_with(&mut self, suffix: &str) -> Result<()> {
        let mut name = self.name.to_owned();
        name.push('_');
        name.push_str(suffix);
        self.name = name;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_with() -> Result<()> {
        let mut p = Parameter::new("foo".to_string(), "tpe".to_string(), Value::Bool(true));
        assert_eq!(p.name, "foo");
        assert_eq!(p.r#type, "tpe");
        assert_eq!(p.value, true.into());
        p.suffix_params_with("bar")?;
        assert_eq!(p.name, "foo_bar");
        assert_eq!(p.r#type, "tpe");
        assert_eq!(p.value, true.into());
        Ok(())
    }
}
