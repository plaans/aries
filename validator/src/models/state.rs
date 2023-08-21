use crate::utils::extract_bounds;
use anyhow::{bail, ensure, Context, Result};
use malachite::Rational;
use std::fmt::Display;

use im::HashMap;

use super::value::Value;

/* ========================================================================== */
/*                                    State                                   */
/* ========================================================================== */

/// Represents the current state of the world during the validation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State(HashMap<Vec<Value>, Value>);

impl State {
    /// Bounds a fluent to a value.
    ///
    /// Returns an error if the value is out of the bounds of the fluent
    pub fn bound(&mut self, k: Vec<Value>, v: Value) -> Result<Option<Value>> {
        let f = k.first().context("Fluent with an empty signature")?;
        match f {
            Value::Symbol(s) => {
                let opt_bounds = extract_bounds(s)?;
                if let Some((lb, ub)) = opt_bounds {
                    match v.clone() {
                        Value::Number(v, _, _) => {
                            let r_lb: Rational = lb.into();
                            let r_ub: Rational = ub.into();
                            ensure!(r_lb <= v && v <= r_ub);
                        }
                        _ => bail!("Try to set a not number value into a fluent with bounds"),
                    }
                };
                Ok(self.0.insert(k, v))
            }
            _ => bail!("Fluent without a symbol as name"),
        }
    }

    /// Returns a reference to the value corresponding to the fluent.
    pub fn get(&self, k: &Vec<Value>) -> Option<&Value> {
        self.0.get(k)
    }
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (fl, v) in self.0.iter() {
            f.write_fmt(format_args!(
                "    {} = {}\n",
                fl.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" "),
                v
            ))?;
        }
        Ok(())
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use im::hashmap;

    use super::*;

    fn k(s: &str) -> Vec<Value> {
        vec![s.into()]
    }
    fn v(b: bool) -> Value {
        b.into()
    }

    #[test]
    fn default() {
        let s = State::default();
        assert!(s.0.is_empty());
    }

    #[test]
    fn bound() -> Result<()> {
        let expected = hashmap! {k("s") => v(true)};
        let mut s = State::default();
        s.bound(k("s"), v(true))?;
        assert_eq!(s.0, expected);
        Ok(())
    }

    #[test]
    fn bound_with_bounds() -> Result<()> {
        let f = k("s - integer[0, 10]");
        let mut s = State::default();

        assert!(s.bound(f.clone(), 15.into()).is_err());
        assert!(s.0.is_empty());
        s.bound(f.clone(), 5.into())?;
        assert_eq!(s.0, hashmap! {f => 5.into()});
        Ok(())
    }

    #[test]
    fn get() -> Result<()> {
        let mut s = State::default();
        s.bound(k("s"), v(true))?;
        assert_eq!(s.get(&k("s")), Some(&v(true)));
        assert_eq!(s.get(&k("a")), None);
        Ok(())
    }
}
