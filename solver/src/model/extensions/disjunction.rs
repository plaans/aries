use crate::core::*;
use crate::model::extensions::PartialBoolAssignment;

/// Extension trait that provides convenience methods to query the status of disjunctions.
pub trait DisjunctionExt<Disj>
where
    Disj: IntoIterator<Item = Lit>,
{
    fn entails(&self, literal: Lit) -> bool;
    fn value(&self, literal: Lit) -> Option<bool>;

    fn presence(&self, literal: Lit) -> Lit;

    fn value_of_clause(&self, disjunction: Disj) -> Option<bool> {
        let mut found_undef = false;
        for disjunct in disjunction.into_iter() {
            match self.value(disjunct) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => found_undef = true,
            }
        }
        if found_undef { None } else { Some(false) }
    }

    // =========== Clauses ============

    fn entailed_clause(&self, disjuncts: Disj) -> bool {
        disjuncts.into_iter().any(|l| self.entails(l))
    }
    fn violated_clause(&self, disjuncts: Disj) -> bool {
        disjuncts.into_iter().all(|l| self.entails(!l))
    }
    fn pending_clause(&self, disjuncts: Disj) -> bool {
        let mut disjuncts = disjuncts.into_iter();
        while let Some(lit) = disjuncts.next() {
            if self.entails(lit) {
                return false;
            }
            if !self.entails(!lit) {
                // pending literal
                return disjuncts.all(|lit| !self.entails(lit));
            }
        }
        false
    }

    fn fusable(&self, l1: Lit, l2: Lit) -> bool {
        l1 == !self.presence(l2) || l2 == !self.presence(l1)
    }
    fn unit_clause(&self, disjuncts: Disj) -> bool {
        let mut disjuncts = disjuncts.into_iter();
        while let Some(lit) = disjuncts.next() {
            if self.entails(lit) {
                return false;
            }
            if !self.entails(!lit) {
                let pending = lit;
                // pending literal, all others should be false
                // there should be at most one other literal that is both unset and fusable
                let mut count = 0;
                for other in disjuncts {
                    if !self.entails(!other) {
                        // literal is not falsified
                        if self.fusable(pending, other) {
                            // we can have at most one of those
                            count += 1;
                            if count > 1 {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                }
                return true;
            }
        }
        // no pending literals founds, clause is not unit
        false
    }
}

impl<Disj: IntoIterator<Item = Lit>, State: PartialBoolAssignment> DisjunctionExt<Disj> for State {
    fn entails(&self, literal: Lit) -> bool {
        self.entails(literal)
    }
    fn value(&self, literal: Lit) -> Option<bool> {
        self.value(literal)
    }

    fn presence(&self, literal: Lit) -> Lit {
        self.presence_literal(literal.variable())
    }
}
