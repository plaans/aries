use aries_datalog::{
    Arg as DatalogArg, Program as DatalogProgram, Rule as DatalogRule, Sym as DatalogSym, VarTable as DatalogPredicate,
};

use aries_solver::core::IntCst;
use idmap::DirectIdMap;

use std::collections::HashMap;

use crate::{TaskId, ext::ground::SourceGrounding};

use super::types::*;

/// Used to extract the facts derived by the datalog engine and decode source groundings from them.
pub(super) struct GrounderProgramInnerResult {
    var_tables: Vec<DatalogPredicate>,
    predicate_id_to_table_map: HashMap<GrounderPredicateId, usize>,
    cst_of_datalog_sym: DirectIdMap<u32, IntCst>,
}

impl GrounderProgramInnerResult {
    pub fn extract_groundings_of_concrete_source(&self, task_id: TaskId) -> Vec<SourceGrounding> {
        let predicate_index = *self
            .predicate_id_to_table_map
            .get(&GrounderPredicateId::ActionApplicable(task_id))
            .unwrap();
        self.var_tables[predicate_index]
            .extract()
            .rows()
            .map(|row| SourceGrounding::from(row.iter().map(|&u| self.cst_of_datalog_sym[u]).collect()))
            .collect()
    }
}

/// Low-level wrapper around the datalog engine.
///
/// Keeps a map from ids of the grounder program's predicates into indices of predicates in the engine itself.
/// Also maintains a (two-way) mapping between constants (integers) encountered in
/// the grounder program and their internal representation in the engine itself.
pub(super) struct GrounderProgramInner {
    datalog_program: DatalogProgram,

    predicates: HashMap<GrounderPredicateId, usize>,

    datalog_sym_of_cst: HashMap<IntCst, u32>,
    cst_of_datalog_sym: DirectIdMap<u32, IntCst>,
    last_datalog_sym_of_cst: u32,
}

impl GrounderProgramInner {
    pub fn new_empty() -> Self {
        Self {
            datalog_program: Default::default(),
            predicates: Default::default(),
            datalog_sym_of_cst: Default::default(),
            cst_of_datalog_sym: Default::default(),
            last_datalog_sym_of_cst: Default::default(),
        }
    }

    pub fn run_and_consume(self) -> GrounderProgramInnerResult {
        GrounderProgramInnerResult {
            var_tables: self.datalog_program.run(),
            predicate_id_to_table_map: self.predicates,
            cst_of_datalog_sym: self.cst_of_datalog_sym,
        }
    }

    pub fn add_fact(&mut self, fact: &GrounderFact) {
        let datalog_predicate_id = &fact.0.grounder_predicate_id;
        let terms = &fact.0.terms;

        debug_assert!(terms.iter().all(|t| matches!(t, GrounderTerm::Cst(_))));

        let row = terms
            .iter()
            .map(|t| {
                if let GrounderTerm::Cst(c) = t {
                    self.get_or_intern_datalog_sym_of_cst(*c)
                } else {
                    unreachable!()
                }
            })
            .collect::<Vec<_>>();

        self.get_or_intern_datalog_predicate_mut(datalog_predicate_id, terms.len())
            .add(row);
    }

    pub fn add_rule(&mut self, rule: &GrounderRule) {
        let head = &rule.head;
        let body = &rule.body;

        let terms = head
            .terms
            .iter()
            .map(|t| match t {
                GrounderTerm::Var(v) => DatalogArg::Var(v.to_u32()),
                GrounderTerm::Cst(c) => DatalogArg::Sym(self.get_or_intern_datalog_sym_of_cst(*c)),
            })
            .collect::<Vec<_>>();
        let head = self
            .get_or_intern_datalog_predicate(&head.grounder_predicate_id, head.terms.len())
            .apply(terms);

        let body = body
            .iter()
            .map(|atom| {
                let terms = atom
                    .terms
                    .iter()
                    .map(|t| match t {
                        GrounderTerm::Var(v) => DatalogArg::Var(v.to_u32()),
                        GrounderTerm::Cst(c) => DatalogArg::Sym(self.get_or_intern_datalog_sym_of_cst(*c)),
                    })
                    .collect::<Vec<_>>();
                self.get_or_intern_datalog_predicate(&atom.grounder_predicate_id, atom.terms.len())
                    .apply(terms)
            })
            .collect::<Vec<_>>();

        self.datalog_program.add_rule(DatalogRule::new(head, body));
    }

    fn get_or_intern_datalog_predicate(
        &mut self,
        predicate_id: &GrounderPredicateId,
        arity: usize,
    ) -> &DatalogPredicate {
        if !self.predicates.contains_key(predicate_id) {
            self.predicates
                .insert(predicate_id.clone(), self.datalog_program.num_predicates());
            self.datalog_program.new_predicate(arity);
        }
        let res = self
            .datalog_program
            .get_predicate(self.predicates[predicate_id])
            .unwrap();
        assert!(res.arity() == arity);
        res
    }

    fn get_or_intern_datalog_predicate_mut(
        &mut self,
        predicate_id: &GrounderPredicateId,
        arity: usize,
    ) -> &mut DatalogPredicate {
        if !self.predicates.contains_key(predicate_id) {
            self.predicates
                .insert(predicate_id.clone(), self.datalog_program.num_predicates());
            self.datalog_program.new_predicate(arity);
        }
        let res = self
            .datalog_program
            .get_predicate_mut(self.predicates[predicate_id])
            .unwrap();
        assert!(res.arity() == arity);
        res
    }
    fn get_or_intern_datalog_sym_of_cst(&mut self, cst: IntCst) -> DatalogSym {
        let res = *self
            .datalog_sym_of_cst
            .entry(cst)
            .or_insert(self.last_datalog_sym_of_cst);
        if !self.cst_of_datalog_sym.contains_key(res) {
            self.cst_of_datalog_sym.insert(res, cst);
            self.last_datalog_sym_of_cst += 1;
        }
        res
    }
}
