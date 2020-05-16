use crate::planning::strips::SymId;
use crate::planning::classical::state::{StateDesc, Lit, Operator};
use crate::planning::utils::enumerate;
use streaming_iterator::StreamingIterator;

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



fn ground(template: &ActionTemplate, desc: &StateDesc<String,String>) -> Result<Vec<Operator>, String> {
    let mut res = Vec::new();

    let mut arg_instances = Vec::with_capacity(template.params.len());
    for arg in &template.params {
        let x = desc.table.types.id_of(&arg.tpe).ok_or(format!("Unknown type: {}", &arg.tpe))?;
        arg_instances.push(desc.table.instances_of_type(x));
    }
    let mut params_iter = enumerate(arg_instances);
    while let Some(params) = params_iter.next() {
        let mut name = "(".to_string();
        name.push_str(&template.name);
        for &arg in params {
            name.push(' '); name.push_str(desc.table.symbol(arg));
        }
        name.push(')');

        let mut op = Operator {
            name,
            precond: Vec::new(),
            effects: Vec::new()
        };

        let mut working = Vec::new();

        for p in &template.pre {
            let lit = p.bind(desc, params, &mut working).unwrap();
            op.precond.push(lit);
        }
        for eff in &template.eff {
            let lit = eff.bind(desc, params, &mut working).unwrap();
            op.effects.push(lit);
        }
        res.push(op);
    }

    Ok(res)
}