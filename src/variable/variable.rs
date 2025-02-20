use crate::variable::int_variable::IntVariable;
use crate::variable::bool_variable::BoolVariable;

#[derive(Clone, Eq, Hash, Debug)]
pub enum Variable {
    IntVariable(IntVariable),
    BoolVariable(BoolVariable),
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl From<IntVariable> for Variable {
    fn from(value: IntVariable) -> Self {
        Self::IntVariable(value)
    }
}

impl From<BoolVariable> for Variable {
    fn from(value: BoolVariable) -> Self {
        Self::BoolVariable(value)
    }
}


#[cfg(test)]
mod tests {
    use crate::domain::{IntDomain, IntRange};

    use super::*;

    #[test]
    fn equality() {
        let range = IntRange::new(2, 2).unwrap();
        let domain = IntDomain::IntRange(range);

        let x: Variable = IntVariable::new(domain).into();
        let y = x.clone();

        let a: Variable = BoolVariable.into();
        let b = a.clone();

        let variables = vec![x, y, a, b];

        // Check that all variables are different
        for i in 0..variables.len() {
            for j in 0..variables.len() {
                if i == j {
                    assert_eq!(variables[i], variables[j])
                } else {
                    assert_ne!(variables[i], variables[j])
                }
            }
        }
    }
}