use crate::parameter::Parameter;
use crate::transitive_conversions;
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

transitive_conversions!(ParVar, Variable, SharedIntVariable);
transitive_conversions!(ParVar, Variable, SharedBoolVariable);