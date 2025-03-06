use transitive::Transitive;

use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;
use crate::variable::Variable;


#[derive(Transitive)]
#[transitive(from(BoolVariable, SharedBoolVariable))]
#[transitive(from(IntVariable, SharedIntVariable))]
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum BasicVariable {
    Bool(SharedBoolVariable),
    Int(SharedIntVariable),
}

impl Identifiable for BasicVariable {
    fn id(&self) -> &Id {
        match self {
            BasicVariable::Int(var) => var.id(),
            BasicVariable::Bool(var) => var.id(),
        }
    }
}

impl From<SharedBoolVariable> for BasicVariable {
    fn from(value: SharedBoolVariable) -> Self {
        Self::Bool(value)
    }
}

impl From<SharedIntVariable> for BasicVariable {
    fn from(value: SharedIntVariable) -> Self {
        Self::Int(value)
    }
}

impl TryFrom<Variable> for BasicVariable {
    type Error = anyhow::Error;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Variable::Basic(v) => Ok(v),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}




#[cfg(test)]
mod tests {
    use crate::domain::IntRange;
    use crate::variable::IntVariable;

    use super::*;

    #[test]
    fn equality() {
        let range_x = IntRange::new(1,4).unwrap();
        let x: BasicVariable = IntVariable::new(
            "x".to_string(),
            range_x.into(),
        ).into();

        let range_y = IntRange::new(2,9).unwrap();
        let y: BasicVariable = IntVariable::new(
            "y".to_string(),
            range_y.into(),
        ).into();

        let a: BasicVariable = BoolVariable::new("a".to_string()).into();
        let b: BasicVariable = BoolVariable::new("b".to_string()).into();

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