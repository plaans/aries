use std::{
    fmt::{Debug, Display},
    ops::{Add, Mul, Neg, Sub},
    str::FromStr,
};

use num_rational::Rational64;
use smallvec::{SmallVec, smallvec};

/// A real-numbered value, internally represented as a rational.
///
/// It mostly exist to hide implementation details from the underlying implementation (that may change in the future)
/// and customize the parsing and printing.
#[derive(Copy, Clone, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub struct RealValue(Rational64);

impl RealValue {
    pub const ZERO: RealValue = RealValue(Rational64::ZERO);

    pub fn new(numer: i64, denom: i64) -> Self {
        RealValue(Rational64::new(numer, denom))
    }

    pub fn from_integer(int: i64) -> Self {
        RealValue(Rational64::from_integer(int))
    }
    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    pub fn numer(&self) -> &i64 {
        self.0.numer()
    }
    pub fn denom(&self) -> &i64 {
        self.0.denom()
    }
}
impl Display for RealValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut numer = *self.numer();
        let mut denom = *self.denom();

        if denom == 1 {
            // happy fast path: just an integer
            return write!(f, "{numer}");
        }

        // that's a rational, we can represent it with a decimal point if the denominator
        // is composed only of 2 and 5 factors
        let mut num_2_factors = 0;
        let mut num_5_factors = 0;
        while denom % 2 == 0 {
            denom /= 2;
            num_2_factors += 1;
        }
        while denom % 5 == 0 {
            denom /= 5;
            num_5_factors += 1;
        }
        if denom != 1 {
            // denominator's decomposition includes factor that are neither 2 nor 5
            // cannot be represented as a decimal number, print it as a rational
            return write!(f, "{}", self.0);
        }
        // we have only 2 and 5 factors
        while num_2_factors < num_5_factors {
            // more 5 than 2, multiply by 2
            num_2_factors += 1;
            numer *= 2;
        }
        while num_5_factors < num_2_factors {
            // more 2 than 5, multiply by 5
            num_5_factors += 1;
            numer *= 5;
        }

        debug_assert_eq!(num_2_factors, num_5_factors);
        debug_assert_eq!(self, &RealValue::new(numer, 10i64.pow(num_5_factors)));

        let mut post_dot_digits = num_2_factors as i32;

        // write the digits's ASCII code in reverse order here
        let mut digits: SmallVec<[u8; 30]> = smallvec![];

        let positive = numer > 0;
        numer = numer.abs();

        while numer > 0 {
            let digit = (numer % 10) as u8;
            numer /= 10;
            digits.push(b'0' + digit); // digit 0 is 48 in ASCII
            post_dot_digits -= 1;
            if post_dot_digits == 0 {
                digits.push(b'.');
                if numer == 0 {
                    digits.push(b'0'); // add 0 if we have nothing on the integer part
                }
            }
        }
        if !positive {
            digits.push(b'-');
        }

        for ascii_digit in digits.into_iter().rev() {
            write!(f, "{}", ascii_digit as char)?;
        }
        Ok(())
    }
}

impl Debug for RealValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl Neg for RealValue {
    type Output = RealValue;

    fn neg(self) -> Self::Output {
        RealValue(-self.0)
    }
}

impl Sub for RealValue {
    type Output = RealValue;

    fn sub(self, rhs: Self) -> Self::Output {
        RealValue(self.0 - rhs.0)
    }
}

impl Add for RealValue {
    type Output = RealValue;

    fn add(self, rhs: Self) -> Self::Output {
        RealValue(self.0 + rhs.0)
    }
}

impl Mul for RealValue {
    type Output = RealValue;

    fn mul(self, rhs: Self) -> Self::Output {
        RealValue(self.0 * rhs.0)
    }
}
impl Mul<i64> for RealValue {
    type Output = RealValue;

    fn mul(self, rhs: i64) -> Self::Output {
        RealValue(self.0 * rhs)
    }
}

impl From<i64> for RealValue {
    fn from(value: i64) -> Self {
        RealValue(value.into())
    }
}

impl FromStr for RealValue {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: this is a very practical implementation but brings a dependency on `regex` just for this function
        num_rational_parse::RationalParse::from_str_flex(s)
            .map(RealValue)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::expressions::real::RealValue;

    fn check(numer: i64, denom: i64) {
        let r = RealValue::new(numer, denom);
        let serialized = r.to_string();
        println!("{} / {} => {}", r.numer(), r.denom(), serialized);
        let deserialized = RealValue::from_str(&serialized).unwrap();

        assert_eq!(
            r, deserialized,
            "{numer} / {denom} fails round trip. String : {serialized}"
        );
    }

    #[test]
    fn printing() {
        check(2, 5);
        check(232, 10);
        check(0, 12);
        check(-1, 1);
        check(-10, 100);
        check(123, 1212);
        check(-123, 1212);
    }
}
