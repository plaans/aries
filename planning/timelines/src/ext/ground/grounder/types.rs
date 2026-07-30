/// High-level views of types used for the grounder datalog program.
use aries_solver::prelude::*;

use idmap::intid::IntegerId;

use crate::{Sym, TaskId};

/// TODO: anticipate union types and have multiple intervals ?
pub(super) type VarDom = (IntCst, IntCst);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum GrounderPredicateId {
    Domain(VarDom),
    Fluent(Sym),
    ActionApplicable(TaskId),
    Goal,
}
impl std::fmt::Display for GrounderPredicateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (head, tail) = match self {
            Self::Domain((lb, ub)) => ("domain".to_string(), Some(&format!("{lb}_{ub}"))),
            Self::Fluent(s) => ("fluent".to_string(), Some(s)),
            Self::ActionApplicable(task_id) => ("applicable".to_string(), Some(&format!("task_{}", task_id.to_int()))),
            Self::Goal => ("goal".to_string(), None),
        };
        f.write_fmt(format_args!(
            "{head}{}",
            tail.map(|s| ["_", s].concat()).unwrap_or_default()
        ))
    }
}

#[derive(Debug, Clone)]
pub(super) enum GrounderTerm {
    Var(Var),
    Cst(IntCst),
}
impl std::fmt::Display for GrounderTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Var(v) => f.write_fmt(format_args!("?{v:?}")),
            Self::Cst(c) => f.write_fmt(format_args!("{c}")),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct GrounderAtom {
    pub grounder_predicate_id: GrounderPredicateId,
    pub terms: Vec<GrounderTerm>,
}
impl std::fmt::Display for GrounderAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let terms = {
            let s = self.terms.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
            if !s.is_empty() {
                format!("({s})")
            } else {
                Default::default()
            }
        };
        f.write_fmt(format_args!("{}{terms}", self.grounder_predicate_id))
    }
}

#[derive(Debug, Clone)]
pub(super) struct GrounderFact(pub GrounderAtom);

impl std::fmt::Display for GrounderFact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.", self.0))
    }
}

#[derive(Debug, Clone)]
pub(super) struct GrounderRule {
    pub head: GrounderAtom,
    pub body: Vec<GrounderAtom>,
}
impl std::fmt::Display for GrounderRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{} :- {}.",
            self.head,
            self.body.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ")
        ))
    }
}

#[derive(Clone)]
pub(super) struct GrounderProgram {
    pub facts: Vec<GrounderFact>,
    pub rules: Vec<GrounderRule>,
}
impl GrounderProgram {
    pub fn new_empty() -> Self {
        Self {
            facts: Default::default(),
            rules: Default::default(),
        }
    }

    pub fn print(&self) {
        for fact in &self.facts {
            println!("{fact}");
        }
        for rules in &self.rules {
            println!("{rules}");
        }
    }

    pub fn add_fact(&mut self, fact: (&GrounderPredicateId, impl AsRef<[GrounderTerm]>)) {
        let atom = GrounderAtom {
            grounder_predicate_id: fact.0.clone(),
            terms: fact.1.as_ref().to_vec(),
        };

        self.facts.push(GrounderFact(atom));
    }

    pub fn add_rule(
        &mut self,
        head: (&GrounderPredicateId, impl AsRef<[GrounderTerm]>),
        body: &[(&GrounderPredicateId, impl AsRef<[GrounderTerm]>)],
    ) {
        let head = GrounderAtom {
            grounder_predicate_id: head.0.clone(),
            terms: head.1.as_ref().to_vec(),
        };
        let body = body
            .iter()
            .map(|pair| GrounderAtom {
                grounder_predicate_id: pair.0.clone(),
                terms: pair.1.as_ref().to_vec(),
            })
            .collect();

        self.rules.push(GrounderRule { head, body });
    }
}
