use std::rc::Rc;

use crate::transitive_conversion;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::Variable;

pub type SharedVariable = Rc<Variable>;

transitive_conversion!(SharedVariable, Variable, IntVariable);
transitive_conversion!(SharedVariable, Variable, BoolVariable);