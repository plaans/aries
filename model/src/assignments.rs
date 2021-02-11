use crate::int_model::IntDomain;
use crate::lang::{Atom, BAtom, BExpr, Bound, IAtom, IVar, IntCst, SAtom, VarRef};
use crate::symbols::SymId;
use crate::symbols::{ContiguousSymbols, SymbolTable};
use crate::Model;

pub trait Assignment {
    fn symbols(&self) -> &SymbolTable;

    fn entails(&self, literal: Bound) -> bool;
    fn value_of_literal(&self, literal: Bound) -> Option<bool> {
        if self.entails(literal) {
            Some(true)
        } else if self.entails(!literal) {
            Some(false)
        } else {
            None
        }
    }
    fn is_undefined_literal(&self, literal: Bound) -> bool {
        self.value_of_literal(literal).is_none()
    }

    fn value_of_clause(&self, disjunction: &[Bound]) -> Option<bool> {
        let mut found_undef = false;
        for disjunct in disjunction {
            match self.value_of_literal(*disjunct) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => found_undef = true,
            }
        }
        if found_undef {
            None
        } else {
            Some(false)
        }
    }

    // =========== Clauses ============

    fn entailed_clause(&self, disjuncts: &[Bound]) -> bool {
        self.value_of_clause(disjuncts) == Some(true)
    }
    fn violated_clause(&self, disjuncts: &[Bound]) -> bool {
        self.value_of_clause(disjuncts) == Some(false)
    }
    fn pending_clause(&self, disjuncts: &[Bound]) -> bool {
        match self.value_of_clause(disjuncts) {
            None => match disjuncts.iter().filter(|l| self.is_undefined_literal(**l)).count() {
                0 => panic!(), // value of clause should have been Some(true)
                1 => false,    // unit
                _ => true,
            },
            _ => false,
        }
    }
    fn unit_clause(&self, disjuncts: &[Bound]) -> bool {
        match self.value_of_clause(disjuncts) {
            None => match disjuncts.iter().filter(|l| self.is_undefined_literal(**l)).count() {
                0 => panic!(), // value of clause should have been Some(true)
                1 => true,     // unit
                _ => false,
            },
            _ => false,
        }
    }

    fn literal_of_expr(&self, expr: BExpr) -> Option<Bound>;

    fn var_domain(&self, var: impl Into<VarRef>) -> &IntDomain;
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

    fn to_owned_assignment(&self) -> SavedAssignment;

    // TODO: added for facilitating the transition of STN
    fn fdist(&self, var: VarRef) -> IntCst {
        self.var_domain(var).ub
    }
    fn bdist(&self, var: VarRef) -> IntCst {
        -self.var_domain(var).lb
    }

    fn lower_bound(&self, int_var: IVar) -> IntCst {
        self.var_domain(int_var).lb
    }

    fn upper_bound(&self, int_var: IVar) -> IntCst {
        self.var_domain(int_var).ub
    }

    fn sym_domain_of(&self, atom: impl Into<SAtom>) -> ContiguousSymbols {
        let atom = atom.into();
        let (lb, ub) = self.int_bounds(atom);
        let lb = lb as usize;
        let ub = ub as usize;
        ContiguousSymbols::new(SymId::from(lb), SymId::from(ub))
    }

    fn sym_value_of(&self, atom: impl Into<SAtom>) -> Option<SymId> {
        self.sym_domain_of(atom).into_singleton()
    }

    /// Returns the value of a boolean atom if it as a set value.
    /// Return None otherwise meaning the value con be
    ///  - either true or false
    ///  - neither true nor false (empty domain)
    fn boolean_value_of(&self, batom: impl Into<BAtom>) -> Option<bool> {
        let batom = batom.into();
        match batom {
            BAtom::Cst(value) => Some(value),
            BAtom::Bound(b) => self.value_of_literal(b),
            BAtom::Expr(e) => self.literal_of_expr(e).and_then(|l| self.value_of_literal(l)),
        }
    }

    /// Return an integer view of the domain of any kind of atom.
    fn int_bounds(&self, atom: impl Into<Atom>) -> (IntCst, IntCst) {
        let atom = atom.into();
        match atom {
            Atom::Bool(atom) => match self.boolean_value_of(atom) {
                Some(true) => (1, 1),
                Some(false) => (0, 0),
                None => (0, 1),
            },
            Atom::Int(atom) => self.domain_of(atom),
            Atom::Sym(atom) => self.domain_of(atom.int_view()),
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
