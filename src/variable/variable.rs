use crate::traits::Identifiable;
use crate::transitive_conversion;
use crate::types::Id;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Variable {
    Bool(SharedBoolVariable),
    Int(SharedIntVariable),
}

impl Identifiable for Variable {
    fn id(&self) -> &Id {
        match self {
            Variable::Int(var) => var.id(),
            Variable::Bool(var) => var.id(),
        }
    }
}

impl From<SharedBoolVariable> for Variable {
    fn from(value: SharedBoolVariable) -> Self {
        Self::Bool(value)
    }
}

impl From<SharedIntVariable> for Variable {
    fn from(value: SharedIntVariable) -> Self {
        Self::Int(value)
    }
}

transitive_conversion!(Variable, SharedBoolVariable, BoolVariable);
transitive_conversion!(Variable, SharedIntVariable, IntVariable);


#[cfg(test)]
mod tests {
    use crate::domain::IntRange;
    use crate::variable::IntVariable;

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