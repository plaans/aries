use crate::planning::strips::SymId;
use crate::planning::classical::state::{StateDesc, Lit};

pub mod state;
pub mod heuristics;
pub mod search;



#[derive(Copy,Clone,Debug)]
pub  enum ParamOrSym {
    Sym(SymId),
    Param(u32)
}
#[derive(Debug)]
pub struct ParameterizedPred {
    pub positive: bool,
    pub sexpr: Vec<ParamOrSym>
}


impl ParameterizedPred {

    pub fn bind<T,S>(&self, sd: &StateDesc<T,S>, params: &[SymId], working: &mut Vec<SymId>) -> Option<Lit> {
        working.clear();
        for &x in &self.sexpr {
            let sym = match x {
                ParamOrSym::Param(i) => params[i as usize],
                ParamOrSym::Sym(s) => s
            };
            working.push(sym);
        }
        sd.sv_id(working.as_slice())
            .map(|sv| Lit::new(sv, self.positive))

    }
}

#[derive(Copy, Clone,Ord, PartialOrd, Eq, PartialEq)]
struct PredId(u32);

impl Into<usize> for PredId {
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl From<usize> for PredId {
    fn from(i: usize) -> Self {
        PredId(i as u32)
    }
}


#[derive(Debug,Clone)]
pub struct Arg {
    pub name: String,
    pub tpe: String
}

pub struct ActionTemplate {
    pub name: String,
    pub params: Vec<Arg>,
    pub pre: Vec<ParameterizedPred>,
    pub eff: Vec<ParameterizedPred>,
}

