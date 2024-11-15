use itertools::Itertools;

use crate::backtrack::{DecLvl, EventIndex};
use crate::core::literals::LitSet;
use crate::core::state::{Domains, Explainer, Origin};
use crate::core::Lit;
use std::collections::HashSet;

use super::literals::Disjunction;

/// Minimizes the clause by removing any redundant literals.
///
/// This corresponds to clause minimization.
/// ref: Efficient All-UIP Learned Clause Minimization, Fleury and Biere (SAT21)
pub fn minimize_clause(clause: Disjunction, doms: &Domains, explainer: &mut impl Explainer) -> Disjunction {
    // preprocessed the elemends of the clause so that we have their negation and their order by the inference time
    // as a side effect, they are grouped by decision level
    let elems = clause
        .literals()
        .iter()
        .copied()
        .map(|l| Elem {
            lit: !l,
            index: doms.implying_event(!l).unwrap(),
            dl: doms.entailing_level(!l),
        })
        .sorted_by_key(|e| e.index)
        .collect_vec();

    // decisions levels for which at least one element of the clause appears
    let decision_levels: HashSet<_> = elems.iter().map(|e| e.dl).collect();
    let mut res = State::default();

    // iterate on all literals, grouped by decision level
    let mut next = 0;
    while next < elems.len() {
        // create a slice of all elements on the same decision level
        let first_on_level = elems[next];
        let level = first_on_level.dl;
        let mut last_on_level = next;
        while last_on_level + 1 < elems.len() && elems[last_on_level + 1].dl == level {
            last_on_level += 1;
        }
        let on_level = &elems[next..=last_on_level];

        // TODO: attempt to shrink to UIP

        // minimize the element on the decision level
        for (i, e) in on_level.iter().copied().enumerate() {
            let l = e.lit;
            if i == 0 {
                // first on level, cannot be redundant
                res.add(l);
            } else {
                let redundant = res.check_redundant(l, doms, explainer, &decision_levels);
                if !redundant {
                    // literal is not redundant, add it to the clause
                    res.add(l);
                }
            }
        }

        // proceed to first element of next decision level
        next = last_on_level + 1;
    }

    Disjunction::new(res.clause)
}

#[derive(Clone, Copy)]
struct Elem {
    pub lit: Lit,
    pub index: EventIndex,
    pub dl: DecLvl,
}

#[derive(Default)]
struct State {
    clause: Vec<Lit>,
    /// literals {l1, ..., ln} such that C => !li
    redundant: LitSet,
    /// literals {l1, ..., ln} such that C =/=> !li
    not_redundant_negs: LitSet,
    queue: Vec<(u32, Lit)>,
}
impl State {
    /// Add a literal to the clause (marking i as redundant for future calls)
    pub fn add(&mut self, l: Lit) {
        self.clause.push(!l);
        self.mark_redundant(l);
    }
    /// Records that C => l
    pub fn mark_redundant(&mut self, l: Lit) {
        self.redundant.insert(l)
    }
    /// Records that C =/=> l
    pub fn mark_not_redundant(&mut self, l: Lit) {
        // we insert the negation, to facilitate the checking
        self.not_redundant_negs.insert(!l);
    }
    /// Returns true if it it is known that C => l
    pub fn known_redundant(&self, l: Lit) -> bool {
        self.redundant.contains(!l)
    }
    /// Returns true if it it is known that C =/=> l
    pub fn known_not_redundant(&self, l: Lit) -> bool {
        // if this is true, then there is a literal li such that C =/=> li   and !li => !l
        // the latter means that  l => li, thus it can *not* be the case that C => l, otherwise, we would have C => li
        self.not_redundant_negs.contains(!l)
    }
    pub fn redundant_cached(&self, l: Lit) -> Option<bool> {
        if self.known_redundant(l) {
            Some(true)
        } else if self.known_not_redundant(l) {
            Some(false)
        } else {
            None // unknown
        }
    }

    /// Determines whether the literal is redundant or not, searching through all antecedant
    /// whenever necessary
    pub fn check_redundant(
        &mut self,
        l: Lit,
        doms: &Domains,
        explainer: &mut dyn Explainer,
        decision_levels: &HashSet<DecLvl>,
    ) -> bool {
        if let Some(known_result) = self.redundant_cached(l) {
            return known_result;
        }
        // we will go depth first to search for an obviously not redundant literal
        let mut depth = 1;
        // stack of the depth first search. First element is the depth in the search tree second element is the literal we want to classify
        // note: we only reuse the queue to avoid reallocation
        self.queue.clear();
        self.queue.push((1, l));

        // we do a depth first search until we find a non-redundant or exhaust the space
        //  - if we find a non-redundant, we will immediatly unwind the stack, mark all its parents as non-redudant as well and return false
        //  - when a literal is shown redundant it is removed from the queue
        //  - if a literal is in an unknown state we add all its implicants to the queue. If we come back to it (tree depth decreased) it means
        //    all its children were shown redundant and we can classify it as redundant as well
        // We cache all intermediate results to keep the complexity linear in the size of the trail
        loop {
            if let Some((d, cur)) = self.queue.last().copied() {
                if d == depth - 1 {
                    // we have exhausted all children, and all are redundant
                    // (otherwise, we would have unwound the stack completely)
                    self.mark_redundant(l);
                    depth -= 1;
                    self.queue.pop();
                } else {
                    debug_assert_eq!(d, depth);
                    let status = self.redundant_cached(cur).or_else(|| {
                        // no known classification, try the different rule for immediate classification
                        let dec_lvl = doms.entailing_level(cur);
                        if dec_lvl == DecLvl::ROOT {
                            // entailed at root => redundant
                            self.mark_redundant(cur);
                            Some(true)
                        } else if !decision_levels.contains(&dec_lvl) {
                            // no literals from the clause on this decision level => not redundant
                            self.mark_not_redundant(cur);
                            Some(false)
                        } else {
                            let Some(event) = doms.implying_event(cur) else {
                                unreachable!()
                            };
                            let event = doms.get_event(event);
                            if event.cause == Origin::DECISION {
                                // decision => not redundant
                                self.mark_not_redundant(cur);
                                Some(false)
                            } else {
                                // unknown we will need to look at its implicants (that will become children in the search tree)
                                None
                            }
                        }
                    });
                    match status {
                        Some(true) => {
                            // redundant, dequeue
                            self.queue.pop();
                        }
                        Some(false) => {
                            // not redundant => unwind stack, marking all parent as not redundant
                            while let Some((d, l)) = self.queue.pop() {
                                if d == depth - 1 {
                                    // we changed level, this literal is our predecessor and thus not redundant
                                    self.mark_not_redundant(l);
                                    // mark next
                                    depth = d;
                                }
                            }
                            // return final classification
                            break false;
                        }
                        None => {
                            // unknown status, enqueue all antecedants
                            // classification will be made when we come back to it
                            for antecedant in doms.implying_literals(cur, explainer).unwrap() {
                                self.queue.push((depth + 1, antecedant));
                            }
                            depth += 1;
                        }
                    }
                }
            } else {
                // finished depth first search without encountering a single non-redundant
                // classify the literal as redundant
                break true;
            }
        }
    }
}
