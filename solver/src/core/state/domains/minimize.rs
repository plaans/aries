use itertools::Itertools;

use crate::backtrack::{DecLvl, EventIndex};
use crate::core::Lit;
use crate::core::literals::LitSet;
use crate::core::state::{Domains, Explainer, Explanation, Origin};
use std::collections::HashSet;

use super::literals::Disjunction;
use super::state::ExplanationQueue;

/// Minimizes the clause by attempting to derive a UIP for each decision level and
/// otherwise removing any redundant literal from the clause.
///
/// This corresponds to clause minimization and shrinking.
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

        if let Some(uip) = res.shrink(on_level, doms, explainer, &decision_levels) {
            // we have a UIP for this level
            // mark all literals of the level as redundant
            for e in on_level {
                res.mark_redundant(e.lit);
            }
            res.add(uip);
        } else {
            // shrinking to UIP failed, minimize all literals of the level
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
        }

        // proceed to first element of next decision level
        next = last_on_level + 1;
    }

    Disjunction::from_vec(res.clause)
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
    uip_queue: ExplanationQueue,
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
                            if matches!(event.cause, Origin::DECISION | Origin::ASSUMPTION) {
                                // decision or assumption => not redundant
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

    /// Attempts to find a single UIP for all literals on the decision level
    pub fn shrink(
        &mut self,
        lits: &[Elem],
        doms: &Domains,
        explainer: &mut dyn Explainer,
        decision_levels: &HashSet<DecLvl>,
    ) -> Option<Lit> {
        if lits.len() == 1 {
            return Some(lits[0].lit);
        }
        debug_assert!(lits.len() > 1);
        debug_assert!(lits.iter().all(|l| doms.entails(l.lit)));
        let decision_level = lits[0].dl;
        debug_assert!(lits.iter().all(|e| e.dl == decision_level));

        let mut explanation = Explanation::with_capacity(64);
        for e in lits {
            explanation.push(e.lit);
        }

        // literals falsified at the current decision level, we need to proceed until there is a single one left (1UIP)
        self.uip_queue.clear();
        // literals that are beyond the current decision level and will be part of the final clause

        loop {
            for l in explanation.lits.drain(..) {
                debug_assert!(doms.entails(l));
                // find the location of the event that made it true
                // if there is no such event, it means that the literal is implied in the initial state and we can ignore it
                if let Some(loc) = doms.implying_event(l) {
                    if doms.trail().decision_level(loc) == decision_level {
                        self.uip_queue.push(loc, l);
                    } else {
                        debug_assert!(doms.trail().decision_level(loc) < decision_level);
                        // check redundant
                        let redundant = self.check_redundant(l, doms, explainer, decision_levels);
                        if !redundant {
                            // we found a non redundant literal from another decision level,
                            // we can bring this to a UIP
                            return None;
                        } else {
                            // redundant literal, just ignore it and proceed
                        }
                    }
                }
            }
            debug_assert!(explanation.lits.is_empty());
            debug_assert!(!self.uip_queue.is_empty());

            // not reached the first UIP yet,
            // select latest falsified literal from queue
            let (l, _) = self.uip_queue.pop().unwrap();

            if self.uip_queue.is_empty() {
                // We have reached the first Unique Implication Point (UIP)
                // the content of result is a conjunction of literal that imply `!l`
                // build the conflict clause and exit
                return Some(l);
            }

            // we necessarily have antecedants because the literal cannot be a decision
            // (otherwise we would have detected that we are at the UIP before)
            // TODO: this allcate a vector on each call
            let antecedants = doms.implying_literals(l, explainer).unwrap();

            for antecedant in antecedants {
                explanation.push(antecedant)
            }
        }
    }
}
