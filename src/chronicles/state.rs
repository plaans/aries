use crate::chronicles::strips::{SymbolTable, SymId, Instances};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use crate::chronicles::enumerate::enumerate;
use streaming_iterator::StreamingIterator;

// TODO: use trait instead of this dummy data structure
pub struct PredicateDesc<T,Sym> {
    name: Sym,
    types: Vec<T>
}

pub struct State<T,I> {
    table: SymbolTable<T,I>,
    state_vars: HashMap<Box<[SymId]>, SV>
}

#[derive(Copy, Clone)]
pub struct SV(usize);

impl<T, Sym> State<T, Sym> where
    T: Clone + Eq + Hash + Display,
    Sym: Clone + Eq + Hash + Display
{

    pub fn new(table: SymbolTable<T, Sym>, predicates: Vec<PredicateDesc<T,Sym>>) -> Result<Self, String> {
        let mut s = State {
            table,
            state_vars: Default::default()
        };

        for pred in predicates {
            let mut generators = Vec::with_capacity(1 + pred.types.len());
            let pred_id = s.table.id(&pred.name)
                .ok_or(format!("unrecorded pred: {}", pred.name))?;

            generators.push(Instances::singleton(pred_id));
            for tpe in pred.types {
                let tpe_id = s.table.types.id_of(&tpe)
                    .ok_or(format!("unknown type: {}", tpe))?;
                generators.push(s.table.instances_of_type(tpe_id));
            }

            let mut iter = enumerate(generators);
            while let Some(sv) = iter.next() {
                let id = SV(s.state_vars.len());
                let cpy: Box<[SymId]> = sv.into();
                assert!(!s.state_vars.contains_key(&cpy));
                s.state_vars.insert(cpy, id);
            }
        }

        Ok(s)
    }

    fn sv_id(&self, sv: &[SymId]) -> Option<SV> {
        self.state_vars.get(sv).copied()
    }

}