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
    pub fn bound(&mut self, k: Vec<Value>, v: Value) -> Option<Value> {
        self.0.insert(k, v)
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
    fn bound() {
        let expected = hashmap! {k("s") => v(true)};
        let mut s = State::default();
        s.bound(k("s"), v(true));
        assert_eq!(s.0, expected);
    }

    #[test]
    fn get() {
        let mut s = State::default();
        s.bound(k("s"), v(true));
        assert_eq!(s.get(&k("s")), Some(&v(true)));
        assert_eq!(s.get(&k("a")), None);
    }
}
