use crate::model::int_model::IntDomain;
use crate::model::lang::{BVar, IVar, IntCst};
use crate::model::Model;
use aries_collections::ref_store::{RefMap, RefVec};
use aries_sat::all::BVar as SatVar;
use aries_sat::all::Lit;

pub trait Assignment {
    fn literal_of(&self, bool_var: BVar) -> Option<Lit>;
    fn value_of_sat_variable(&self, sat_variable: SatVar) -> Option<bool>;
    fn domain_of(&self, int_var: IVar) -> &IntDomain;

    fn to_owned(&self) -> SavedAssignment;

    fn literal_value(&self, literal: Lit) -> Option<bool> {
        self.value_of_sat_variable(literal.variable())
            .map(|value| if literal.value() { value } else { !value })
    }

    fn lower_bound(&self, int_var: IVar) -> IntCst {
        self.domain_of(int_var).lb
    }

    fn upper_bound(&self, int_var: IVar) -> IntCst {
        self.domain_of(int_var).ub
    }
}

#[derive(Clone)]
pub struct SavedAssignment {
    bool_mapping: RefMap<BVar, Lit>,
    bool_values: RefMap<SatVar, bool>,
    int_domains: RefVec<IVar, IntDomain>,
}

impl SavedAssignment {
    pub fn from_model(model: &Model) -> SavedAssignment {
        SavedAssignment {
            bool_mapping: model.bools.binding.clone(),
            bool_values: model.bools.values.clone(),
            int_domains: model.ints.domains.clone(),
        }
    }
}

impl Assignment for SavedAssignment {
    fn literal_of(&self, bool_var: BVar) -> Option<Lit> {
        self.bool_mapping.get(bool_var).copied()
    }

    fn value_of_sat_variable(&self, sat_variable: SatVar) -> Option<bool> {
        self.bool_values.get(sat_variable).copied()
    }

    fn domain_of(&self, int_var: IVar) -> &IntDomain {
        &self.int_domains[int_var]
    }

    fn to_owned(&self) -> SavedAssignment {
        self.clone()
    }
}
