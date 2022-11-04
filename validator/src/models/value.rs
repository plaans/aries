use std::ops::{Add, BitAnd, BitOr, Div, Mul, Not, Sub};

use anyhow::{bail, Ok, Result};
use malachite::Rational;

/// Represents the value of an expression after its evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Bool(bool),
    Number(Rational),
    Symbol(String),
}

impl Add for &Value {
    type Output = Result<Value>;

    fn add(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok(Value::Number(v1 + v2)),
                _ => bail!("Add operation with a non-number value"),
            },
            _ => bail!("Add operation with a non-number value"),
        }
    }
}

impl Sub for &Value {
    type Output = Result<Value>;

    fn sub(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok(Value::Number(v1 - v2)),
                _ => bail!("Sub operation with a non-number value"),
            },
            _ => bail!("Sub operation with a non-number value"),
        }
    }
}

impl Mul for &Value {
    type Output = Result<Value>;

    fn mul(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok(Value::Number(v1 * v2)),
                _ => bail!("Mul operation with a non-number value"),
            },
            _ => bail!("Mul operation with a non-number value"),
        }
    }
}

impl Div for &Value {
    type Output = Result<Value>;

    fn div(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok(Value::Number(v1 / v2)),
                _ => bail!("Div operation with a non-number value"),
            },
            _ => bail!("Div operation with a non-number value"),
        }
    }
}

impl BitAnd for &Value {
    type Output = Result<Value>;

    fn bitand(self, rhs: Self) -> Self::Output {
        match self {
            Value::Bool(v1) => match rhs {
                Value::Bool(v2) => Ok(Value::Bool(v1 & v2)),
                _ => bail!("BitAnd operation with a non-boolean value"),
            },
            _ => bail!("BitAnd operation with a non-boolean value"),
        }
    }
}

impl BitOr for &Value {
    type Output = Result<Value>;

    fn bitor(self, rhs: Self) -> Self::Output {
        match self {
            Value::Bool(v1) => match rhs {
                Value::Bool(v2) => Ok(Value::Bool(v1 | v2)),
                _ => bail!("BitOr operation with a non-boolean value"),
            },
            _ => bail!("BitOr operation with a non-boolean value"),
        }
    }
}

impl Not for &Value {
    type Output = Result<Value>;

    fn not(self) -> Self::Output {
        match self {
            Value::Bool(v) => Ok(Value::Bool(!v)),
            _ => bail!("Not operation with a non-boolean value"),
        }
    }
}

impl Not for Value {
    type Output = Result<Value>;

    fn not(self) -> Self::Output {
        !(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add() -> Result<()> {
        let b = Value::Bool(true);
        let s = Value::Symbol("s".to_string());
        let n1 = Value::Number(1.into());
        let n2 = Value::Number(2.into());
        assert!((&b + &b).is_err());
        assert!((&b + &s).is_err());
        assert!((&b + &n1).is_err());
        assert!((&s + &b).is_err());
        assert!((&s + &s).is_err());
        assert!((&s + &n1).is_err());
        assert!((&n2 + &b).is_err());
        assert!((&n2 + &s).is_err());
        assert_eq!((&n2 + &n1)?, Value::Number(3.into()));
        Ok(())
    }

    #[test]
    fn sub() -> Result<()> {
        let b = Value::Bool(true);
        let s = Value::Symbol("s".to_string());
        let n1 = Value::Number(1.into());
        let n2 = Value::Number(4.into());
        assert!((&b - &b).is_err());
        assert!((&b - &s).is_err());
        assert!((&b - &n1).is_err());
        assert!((&s - &b).is_err());
        assert!((&s - &s).is_err());
        assert!((&s - &n1).is_err());
        assert!((&n2 - &b).is_err());
        assert!((&n2 - &s).is_err());
        assert_eq!((&n2 - &n1)?, Value::Number(3.into()));
        Ok(())
    }

    #[test]
    fn mul() -> Result<()> {
        let b = Value::Bool(true);
        let s = Value::Symbol("s".to_string());
        let n1 = Value::Number(2.into());
        let n2 = Value::Number(4.into());
        assert!((&b * &b).is_err());
        assert!((&b * &s).is_err());
        assert!((&b * &n1).is_err());
        assert!((&s * &b).is_err());
        assert!((&s * &s).is_err());
        assert!((&s * &n1).is_err());
        assert!((&n2 * &b).is_err());
        assert!((&n2 * &s).is_err());
        assert_eq!((&n2 * &n1)?, Value::Number(8.into()));
        Ok(())
    }

    #[test]
    fn div() -> Result<()> {
        let b = Value::Bool(true);
        let s = Value::Symbol("s".to_string());
        let n1 = Value::Number(2.into());
        let n2 = Value::Number(6.into());
        assert!((&b / &b).is_err());
        assert!((&b / &s).is_err());
        assert!((&b / &n1).is_err());
        assert!((&s / &b).is_err());
        assert!((&s / &s).is_err());
        assert!((&s / &n1).is_err());
        assert!((&n2 / &b).is_err());
        assert!((&n2 / &s).is_err());
        assert_eq!((&n2 / &n1)?, Value::Number(3.into()));
        Ok(())
    }

    #[test]
    fn bitand() -> Result<()> {
        let bt = Value::Bool(true);
        let bf = Value::Bool(false);
        let s = Value::Symbol("s".to_string());
        let n = Value::Number(2.into());
        assert_eq!((&bt & &bt)?, Value::Bool(true));
        assert_eq!((&bt & &bf)?, Value::Bool(false));
        assert_eq!((&bf & &bt)?, Value::Bool(false));
        assert_eq!((&bf & &bf)?, Value::Bool(false));
        assert!((&bt & &s).is_err());
        assert!((&bt & &n).is_err());
        assert!((&s & &bt).is_err());
        assert!((&s & &s).is_err());
        assert!((&s & &n).is_err());
        assert!((&n & &bt).is_err());
        assert!((&n & &s).is_err());
        assert!((&n & &n).is_err());
        Ok(())
    }

    #[test]
    fn bitor() -> Result<()> {
        let bt = Value::Bool(true);
        let bf = Value::Bool(false);
        let s = Value::Symbol("s".to_string());
        let n = Value::Number(2.into());
        assert_eq!((&bt | &bt)?, Value::Bool(true));
        assert_eq!((&bt | &bf)?, Value::Bool(true));
        assert_eq!((&bf | &bt)?, Value::Bool(true));
        assert_eq!((&bf | &bf)?, Value::Bool(false));
        assert!((&bt | &s).is_err());
        assert!((&bt | &n).is_err());
        assert!((&s | &bt).is_err());
        assert!((&s | &s).is_err());
        assert!((&s | &n).is_err());
        assert!((&n | &bt).is_err());
        assert!((&n | &s).is_err());
        assert!((&n | &n).is_err());
        Ok(())
    }

    #[test]
    fn not() -> Result<()> {
        let bt = Value::Bool(true);
        let bf = Value::Bool(false);
        let s = Value::Symbol("s".to_string());
        let n = Value::Number(2.into());
        assert_eq!((!&bt)?, Value::Bool(false));
        assert_eq!((!&bf)?, Value::Bool(true));
        assert!((!&s).is_err());
        assert!((!&n).is_err());
        assert_eq!((!bt)?, Value::Bool(false));
        assert_eq!((!bf)?, Value::Bool(true));
        assert!((!s).is_err());
        assert!((!n).is_err());
        Ok(())
    }
}
