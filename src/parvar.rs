use transitive::Transitive;

use crate::parameter::Parameter;
use crate::variable::BasicVariable;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;
use crate::variable::Variable;

#[derive(Transitive)]
#[transitive(from(BasicVariable, Variable))]
#[transitive(from(SharedBoolVariable, BasicVariable, Variable))]
#[transitive(from(BoolVariable, SharedBoolVariable, BasicVariable, Variable))]
#[transitive(from(SharedIntVariable, BasicVariable, Variable))]
#[transitive(from(IntVariable, SharedIntVariable, BasicVariable, Variable))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParVar {
    Par(Parameter),
    Var(Variable),
}

impl From<Parameter> for ParVar {
    fn from(value: Parameter) -> Self {
        Self::Par(value)
    }
}

impl From<Variable> for ParVar {
    fn from(value: Variable) -> Self {
        Self::Var(value)
    }
}