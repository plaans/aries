use crate::int_model::IntDomain;
use crate::lang::{BAtom, BVar, DVar, IAtom, IVar, IntCst, SAtom, VarOrSym};
use crate::symbols::SymId;
use crate::symbols::{ContiguousSymbols, SymbolTable};
use crate::Model;
use aries_sat::all::BVar as SatVar;
use aries_sat::all::Lit;

pub trait Assignment {
    fn symbols(&self) -> &SymbolTable<String, String> {
        todo!()
    }

    fn literal_of(&self, bool_var: BVar) -> Option<Lit>;
    fn value_of_sat_variable(&self, sat_variable: SatVar) -> Option<bool>;
    fn var_domain(&self, var: impl Into<DVar>) -> &IntDomain;
    fn domain_of(&self, atom: impl Into<IAtom>) -> (IntCst, IntCst) {
        let atom = atom.into();
        let base = atom
            .var
            .map(|v| {
                let d = self.var_domain(v);
                (d.lb, d.ub)
            })
            .unwrap_or((0, 0));
        (base.0 + atom.shift, base.1 + atom.shift)
    }

    fn to_owned(&self) -> SavedAssignment;

    fn literal_value(&self, literal: Lit) -> Option<bool> {
        self.value_of_sat_variable(literal.variable())
            .map(|value| if literal.value() { value } else { !value })
    }

    fn lower_bound(&self, int_var: IVar) -> IntCst {
        self.var_domain(int_var).lb
    }

    fn upper_bound(&self, int_var: IVar) -> IntCst {
        self.var_domain(int_var).ub
    }

    fn sym_domain_of(&self, atom: impl Into<SAtom>) -> ContiguousSymbols {
        let atom = atom.into();
        match atom.atom {
            VarOrSym::Var(v) => {
                let &IntDomain { lb, ub, .. } = self.var_domain(v);
                let lb = lb as usize;
                let ub = ub as usize;
                ContiguousSymbols::new(SymId::from(lb), SymId::from(ub))
            }
            VarOrSym::Sym(s) => ContiguousSymbols::new(s, s),
        }
    }

    fn sym_value_of(&self, atom: impl Into<SAtom>) -> Option<SymId> {
        self.sym_domain_of(atom).into_singleton()
    }

    fn boolean_value_of(&self, batom: impl Into<BAtom>) -> Option<bool> {
        let batom = batom.into();
        match batom.var {
            None => Some(!batom.negated),
            Some(v) => self
                .literal_of(v)
                .and_then(|l| self.literal_value(l))
                .map(|v| if batom.negated { !v } else { v }),
        }
    }
}

// TODO: this is correct but wasteful
pub type SavedAssignment = Model;

// #[derive(Clone)]
// pub struct SavedAssignment {
//     bool_mapping: RefMap<BVar, Lit>,
//     bool_values: RefMap<SatVar, bool>,
//     int_domains: RefVec<DVar, IntDomain>,
// }
//
impl SavedAssignment {
    pub fn from_model(model: &Model) -> SavedAssignment {
        model.clone()
        // SavedAssignment {
        //     bool_mapping: model.discrete.binding.clone(),
        //     bool_values: model.discrete.values.clone(),
        //     int_domains: todo!(), //model.discrete.domains.clone(),
        // }
    }
}
//
// impl Assignment for SavedAssignment {
//     fn literal_of(&self, bool_var: BVar) -> Option<Lit> {
//         self.bool_mapping.get(bool_var).copied()
//     }
//
//     fn value_of_sat_variable(&self, sat_variable: SatVar) -> Option<bool> {
//         self.bool_values.get(sat_variable).copied()
//     }
//
//     fn var_domain(&self, var: impl Into<DVar>) -> &IntDomain {
//         &self.int_domains[var.into()]
//     }
//
//     fn to_owned(&self) -> SavedAssignment {
//         self.clone()
//     }
// }
