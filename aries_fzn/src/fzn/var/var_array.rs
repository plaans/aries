use std::rc::Rc;

use crate::fzn::Fzn;
use crate::fzn::Name;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;

/// Generic array of variables.
#[derive(Clone, Eq, Debug)]
pub struct GenArrayVariable<T> {
    variables: Vec<T>,
    name: String,
    output: bool,
}

impl<T> GenArrayVariable<T> {
    pub fn new(variables: Vec<T>, name: String, output: bool) -> Self {
        Self {
            variables,
            name,
            output,
        }
    }

    pub fn variables(&self) -> impl Iterator<Item = &T> {
        self.variables.iter()
    }

    pub fn len(&self) -> usize {
        self.variables.len()
    }

    pub fn output(&self) -> bool {
        self.output
    }
}

impl<T> Name for GenArrayVariable<T> {
    fn name(&self) -> &String {
        &self.name
    }
}

impl<T: Fzn> Fzn for GenArrayVariable<T> {
    fn fzn(&self) -> String {
        self.variables.fzn()
    }
}

impl<T: PartialEq> PartialEq for GenArrayVariable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.variables == other.variables
    }
}

/// Boolean array variable.
///
/// ```flatzinc
/// array [1..2] of var bool: b;
/// ```
pub type VarBoolArray = GenArrayVariable<Rc<VarBool>>;

/// Integer array variable.
///
/// ```flatzinc
/// array [1..3] of var 2..3: x;
/// ```
pub type VarIntArray = GenArrayVariable<Rc<VarInt>>;
