use crate::parameter::Parameter;
use crate::transitive_conversion;
use crate::variable::BasicVariable;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;
use crate::variable::Variable;

#[derive(Clone)]
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

transitive_conversion!(ParVar, Variable, BasicVariable);
transitive_conversion!(ParVar, BasicVariable, SharedBoolVariable);
transitive_conversion!(ParVar, SharedBoolVariable, BoolVariable);
transitive_conversion!(ParVar, BasicVariable, SharedIntVariable);
transitive_conversion!(ParVar, SharedIntVariable, IntVariable);