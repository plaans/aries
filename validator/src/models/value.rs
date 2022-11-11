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

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Number(i.into())
    }
}

impl From<Rational> for Value {
    fn from(r: Rational) -> Self {
        Value::Number(r)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Symbol(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Symbol(s)
    }
}

impl Add for &Value {
    type Output = Result<Value>;

    fn add(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok((v1 + v2).into()),
                _ => bail!("Add operation with a non-number value"),
            },
            _ => bail!("Add operation with a non-number value"),
        }
    }
}

impl Add for Value {
    type Output = Result<Value>;

    fn add(self, rhs: Self) -> Self::Output {
        &self + &rhs
    }
}

impl Sub for &Value {
    type Output = Result<Value>;

    fn sub(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok((v1 - v2).into()),
                _ => bail!("Sub operation with a non-number value"),
            },
            _ => bail!("Sub operation with a non-number value"),
        }
    }
}

impl Sub for Value {
    type Output = Result<Value>;

    fn sub(self, rhs: Self) -> Self::Output {
        &self - &rhs
    }
}

impl Mul for &Value {
    type Output = Result<Value>;

    fn mul(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok((v1 * v2).into()),
                _ => bail!("Mul operation with a non-number value"),
            },
            _ => bail!("Mul operation with a non-number value"),
        }
    }
}

impl Mul for Value {
    type Output = Result<Value>;

    fn mul(self, rhs: Self) -> Self::Output {
        &self * &rhs
    }
}

impl Div for &Value {
    type Output = Result<Value>;

    fn div(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(v1) => match rhs {
                Value::Number(v2) => Ok((v1 / v2).into()),
                _ => bail!("Div operation with a non-number value"),
            },
            _ => bail!("Div operation with a non-number value"),
        }
    }
}

impl Div for Value {
    type Output = Result<Value>;

    fn div(self, rhs: Self) -> Self::Output {
        &self / &rhs
    }
}

impl BitAnd for &Value {
    type Output = Result<Value>;

    fn bitand(self, rhs: Self) -> Self::Output {
        match self {
            Value::Bool(v1) => match rhs {
                Value::Bool(v2) => Ok((v1 & v2).into()),
                _ => bail!("BitAnd operation with a non-boolean value"),
            },
            _ => bail!("BitAnd operation with a non-boolean value"),
        }
    }
}

impl BitAnd for Value {
    type Output = Result<Value>;

    fn bitand(self, rhs: Self) -> Self::Output {
        &self & &rhs
    }
}

impl BitOr for &Value {
    type Output = Result<Value>;

    fn bitor(self, rhs: Self) -> Self::Output {
        match self {
            Value::Bool(v1) => match rhs {
                Value::Bool(v2) => Ok((v1 | v2).into()),
                _ => bail!("BitOr operation with a non-boolean value"),
            },
            _ => bail!("BitOr operation with a non-boolean value"),
        }
    }
}

impl BitOr for Value {
    type Output = Result<Value>;

    fn bitor(self, rhs: Self) -> Self::Output {
        &self | &rhs
    }
}

impl Not for &Value {
    type Output = Result<Value>;

    fn not(self) -> Self::Output {
        match self {
            Value::Bool(v) => Ok((!v).into()),
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

    macro_rules! test_ref_err {
        ($a:expr, $b:expr, $op:tt) => {
            assert!((&$a $op &$b).is_err());
        };
    }
    macro_rules! test_ref {
        ($a:expr, $b:expr, $e:expr, $op:tt) => {
                assert_eq!((&$a $op &$b)?, $e);
        };
    }
    macro_rules! test_val_err {
        ($a:expr, $b:expr, $op:tt) => {
                assert!(($a.clone() $op $b.clone()).is_err());
        };
    }
    macro_rules! test_val {
        ($a:expr, $b:expr, $e:expr, $op:tt) => {
                assert_eq!(($a.clone() $op $b.clone())?, $e);
        };
    }

    #[test]
    fn from_bool() {
        assert_eq!(Value::Bool(true), true.into());
        assert_eq!(Value::Bool(false), false.into());
    }

    #[test]
    fn from_i64() {
        assert_eq!(Value::Number(5.into()), 5.into());
    }

    #[test]
    fn from_rational() {
        let r = Rational::from_signeds(5, 2);
        assert_eq!(Value::Number(r.clone()), r.into());
    }

    #[test]
    fn from_str() {
        assert_eq!(Value::Symbol("s".into()), "s".into());
    }

    #[test]
    fn from_string() {
        assert_eq!(Value::Symbol("s".into()), "s".to_string().into());
    }

    #[test]
    fn add() -> Result<()> {
        let b: Value = true.into();
        let s: Value = "s".into();
        let n1: Value = 1.into();
        let n2: Value = 2.into();

        // References
        test_ref_err!(b, b, +);
        test_ref_err!(b, s, +);
        test_ref_err!(b, n1, +);
        test_ref_err!(s, b, +);
        test_ref_err!(s, s, +);
        test_ref_err!(s, n1, +);
        test_ref_err!(n2, b, +);
        test_ref_err!(n2, s, +);
        test_ref!(n2, n1, 3.into(), +);
        // Values
        test_val_err!(b, b, +);
        test_val_err!(b, s, +);
        test_val_err!(b, n1, +);
        test_val_err!(s, b, +);
        test_val_err!(s, s, +);
        test_val_err!(s, n1, +);
        test_val_err!(n2, b, +);
        test_val_err!(n2, s, +);
        test_val!(n2, n1, 3.into(), +);
        Ok(())
    }

    #[test]
    fn sub() -> Result<()> {
        let b: Value = true.into();
        let s: Value = "s".into();
        let n1: Value = 1.into();
        let n2: Value = 4.into();

        // References
        test_ref_err!(b, b, -);
        test_ref_err!(b, s, -);
        test_ref_err!(b, n1, -);
        test_ref_err!(s, b, -);
        test_ref_err!(s, s, -);
        test_ref_err!(s, n1, -);
        test_ref_err!(n2, b, -);
        test_ref_err!(n2, s, -);
        test_ref!(n2, n1, 3.into(), -);
        // Values
        test_val_err!(b, b, -);
        test_val_err!(b, s, -);
        test_val_err!(b, n1, -);
        test_val_err!(s, b, -);
        test_val_err!(s, s, -);
        test_val_err!(s, n1, -);
        test_val_err!(n2, b, -);
        test_val_err!(n2, s, -);
        test_val!(n2, n1, 3.into(), -);
        Ok(())
    }

    #[test]
    fn mul() -> Result<()> {
        let b: Value = true.into();
        let s: Value = "s".into();
        let n1: Value = 2.into();
        let n2: Value = 4.into();

        // References
        test_ref_err!(b, b, *);
        test_ref_err!(b, s, *);
        test_ref_err!(b, n1, *);
        test_ref_err!(s, b, *);
        test_ref_err!(s, s, *);
        test_ref_err!(s, n1, *);
        test_ref_err!(n2, b, *);
        test_ref_err!(n2, s, *);
        test_ref!(n2, n1, 8.into(), *);
        // Values
        test_val_err!(b, b, *);
        test_val_err!(b, s, *);
        test_val_err!(b, n1, *);
        test_val_err!(s, b, *);
        test_val_err!(s, s, *);
        test_val_err!(s, n1, *);
        test_val_err!(n2, b, *);
        test_val_err!(n2, s, *);
        test_val!(n2, n1, 8.into(), *);
        Ok(())
    }

    #[test]
    fn div() -> Result<()> {
        let b: Value = true.into();
        let s: Value = "s".into();
        let n1: Value = 2.into();
        let n2: Value = 6.into();

        // References
        test_ref_err!(b, b, /);
        test_ref_err!(b, s, /);
        test_ref_err!(b, n1, /);
        test_ref_err!(s, b, /);
        test_ref_err!(s, s, /);
        test_ref_err!(s, n1, /);
        test_ref_err!(n2, b, /);
        test_ref_err!(n2, s, /);
        test_ref!(n2, n1, 3.into(), /);
        // Values
        test_val_err!(b, b, /);
        test_val_err!(b, s, /);
        test_val_err!(b, n1, /);
        test_val_err!(s, b, /);
        test_val_err!(s, s, /);
        test_val_err!(s, n1, /);
        test_val_err!(n2, b, /);
        test_val_err!(n2, s, /);
        test_val!(n2, n1, 3.into(), /);
        Ok(())
    }

    #[test]
    fn bitand() -> Result<()> {
        let bt: Value = true.into();
        let bf: Value = false.into();
        let s: Value = "s".into();
        let n: Value = 2.into();

        // References
        test_ref!(bt, bt, true.into(), &);
        test_ref!(bt, bf, false.into(), &);
        test_ref!(bf, bt, false.into(), &);
        test_ref!(bf, bf, false.into(), &);
        test_ref_err!(bt, s, &);
        test_ref_err!(bt, n, &);
        test_ref_err!(s, bt, &);
        test_ref_err!(s, s, &);
        test_ref_err!(s, n, &);
        test_ref_err!(n, bt, &);
        test_ref_err!(n, s, &);
        test_ref_err!(n, n, &);
        // Values
        test_val!(bt, bt, true.into(), &);
        test_val!(bt, bf, false.into(), &);
        test_val!(bf, bt, false.into(), &);
        test_val!(bf, bf, false.into(), &);
        test_val_err!(bt, s, &);
        test_val_err!(bt, n, &);
        test_val_err!(s, bt, &);
        test_val_err!(s, s, &);
        test_val_err!(s, n, &);
        test_val_err!(n, bt, &);
        test_val_err!(n, s, &);
        test_val_err!(n, n, &);
        Ok(())
    }

    #[test]
    fn bitor() -> Result<()> {
        let bt: Value = true.into();
        let bf: Value = false.into();
        let s: Value = "s".into();
        let n: Value = 2.into();

        // References
        test_ref!(bt, bt, true.into(), |);
        test_ref!(bt, bf, true.into(), |);
        test_ref!(bf, bt, true.into(), |);
        test_ref!(bf, bf, false.into(), |);
        test_ref_err!(bt, s, |);
        test_ref_err!(bt, n, |);
        test_ref_err!(s, bt, |);
        test_ref_err!(s, s, |);
        test_ref_err!(s, n, |);
        test_ref_err!(n, bt, |);
        test_ref_err!(n, s, |);
        test_ref_err!(n, n, |);
        // Values
        test_val!(bt, bt, true.into(), |);
        test_val!(bt, bf, true.into(), |);
        test_val!(bf, bt, true.into(), |);
        test_val!(bf, bf, false.into(), |);
        test_val_err!(bt, s, |);
        test_val_err!(bt, n, |);
        test_val_err!(s, bt, |);
        test_val_err!(s, s, |);
        test_val_err!(s, n, |);
        test_val_err!(n, bt, |);
        test_val_err!(n, s, |);
        test_val_err!(n, n, |);
        Ok(())
    }

    #[test]
    fn not() -> Result<()> {
        let bt: Value = true.into();
        let bf: Value = false.into();
        let s: Value = "s".into();
        let n: Value = 2.into();

        // References
        assert_eq!((!&bt)?, false.into());
        assert_eq!((!&bf)?, true.into());
        assert!((!&s).is_err());
        assert!((!&n).is_err());
        //Values
        assert_eq!((!bt)?, false.into());
        assert_eq!((!bf)?, true.into());
        assert!((!s).is_err());
        assert!((!n).is_err());
        Ok(())
    }
}
