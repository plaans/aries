use std::rc::Rc;

use transitive::Transitive;

use crate::parameter::Parameter;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::Variable;

// Workaround to transitive crate issue
// https://github.com/bobozaur/transitive/issues/11
type RcIntVariable = Rc<IntVariable>;
type RcBoolVariable = Rc<BoolVariable>;

#[derive(Transitive)]
#[transitive(from(Rc<BoolVariable>, Variable))]
#[transitive(from(BoolVariable, Rc<BoolVariable>, Variable))]
#[transitive(from(Rc<IntVariable>, Variable))]
#[transitive(from(IntVariable, Rc<IntVariable>, Variable))]
#[transitive(try_into(Variable, RcIntVariable))]
#[transitive(try_into(Variable, RcBoolVariable))]
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