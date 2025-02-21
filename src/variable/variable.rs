use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::int_variable::IntVariable;
use crate::variable::bool_variable::BoolVariable;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Variable {
    IntVariable(IntVariable),
    BoolVariable(BoolVariable),
}

impl Identifiable for Variable {
    fn id(&self) -> &Id {
        match self {
            Variable::IntVariable(var) => var.id(),
            Variable::BoolVariable(var) => var.id(),
        }
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
    use crate::domain::IntRange;

    use super::*;

    #[test]
    fn equality() {
        let range_x = IntRange::new(1,4).unwrap();
        let x: Variable = IntVariable::new(
            "x".to_string(),
            range_x.into(),
        ).into();

        let range_y = IntRange::new(2,9).unwrap();
        let y: Variable = IntVariable::new(
            "y".to_string(),
            range_y.into(),
        ).into();

        let a: Variable = BoolVariable::new("a".to_string()).into();
        let b: Variable = BoolVariable::new("b".to_string()).into();

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