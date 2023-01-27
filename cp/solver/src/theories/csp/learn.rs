use super::*;
use crate::theories::csp::range_set::RangeSet;
use crate::theories::csp::signed_literal::SignedLit;
use aries_collections::heap::IdxHeap;
use aries_collections::ref_store::RefMap;
use aries_collections::set::RefSet;
use aries_model::lang::*;
use aries_model::Model;
use std::cmp::Reverse;

type EntryId = usize;

struct Entry {
    /// modified variable
    v: VarRef,
    /// domain after modification
    d: RangeSet,
    cause: Option<CId>,
    // mask
    // m: u32,
    // e: u32
    m: IntEvent,
    /// index in the stack
    i: EntryId,
    /// index of direct predecessor (same variable)
    p: EntryId,
    /// decision level
    dl: usize,
}

enum IntEvent {
    Void,
}

struct LazyImplication<'csp> {
    literals: RefMap<VarRef, SignedLit>,
    entries: Vec<Entry>,
    root_entries: HashMap<VarRef, EntryId>,
    active_entries: usize,
    csp: &'csp CSP,
}

impl LazyImplication<'_> {
    pub fn new<'csp>(model: &Model, csp: &'csp CSP) -> LazyImplication<'csp> {
        let mut res = LazyImplication {
            literals: Default::default(),
            entries: vec![],
            root_entries: Default::default(),
            active_entries: 0,
            csp,
        };
        for var in model.discrete.variables() {
            let (lb, ub) = model.bounds(IVar::new(var));
            let e = Entry {
                v: var,
                d: RangeSet::new(lb, ub),
                cause: None,
                m: IntEvent::Void,
                i: res.active_entries,
                p: res.active_entries,
                dl: 1,
            };
            res.literals.insert(var, SignedLit::new((lb, ub)));
            res.root_entries.insert(var, res.active_entries);
            res.entries.push(e);
            res.active_entries += 1;
        }

        res
    }

    fn collect_nodes_from_conflict(&mut self, cid: CId, var: VarRef, front: &mut Front) {
        let root = self.root_entries[&var];
        let root = &self.entries[root];
        assert_eq!(self.entries[root.p].cause, Some(cid));
        front.put(var, root.p);
    }

    fn predecessors_of(&mut self, p: EntryId, front: &mut Front) {
        let entry = &self.entries[p];
        let cause = entry.cause.unwrap();
        // add the predecessor of 'p'
        front.put(entry.v, entry.p);
        let constraint = &self.csp.immut.constraints[cause];
        constraint.for_each_var(&mut |v| self.find_predecessor(v, p, front));
        todo!()
    }

    /**
     * Find the direct predecessor of a node, declared on variable <i>vi</i>, starting from node at
     * position <i>p</i>.
     * If a variable-based node already exists in <i>front</i>, then this node is used to look for the predecessor,
     * assuming that it is below <i>p</i> (otherwise, this node is the predecessor we are looking for).
     * Otherwise, there is no node based on <i>vi</i> in <i>front</i> and the rightmost node above
     * <i>p</i>, starting from the predecessor of its root node, is added.
     * @param front the set to update
     * @param vi the variable to look the predecessor for
     * @param p the rightmost position of the node (below means outdated node).
     */
    fn find_predecessor(&mut self, vi: VarRef, p: EntryId, front: &mut Front) {
        match front.get_value(vi) {
            Some(mut cpos) => {
                while cpos > p {
                    cpos = self.entries[cpos].p;
                }
                front.replace(vi, cpos);
            }
            None => {
                front.put(vi, self.right_most_node(p, vi));
            }
        }
    }

    /**
     * Find the right-most node, before  <i>p</i>, in this,
     * such that <i>var</i> matches the node.
     * @param var a variable
     * @return right-most position of var between [0,p] in this
     */
    fn right_most_node(&self, limit: EntryId, var: VarRef) -> EntryId {
        // two ways of looking for the node
        // 1. reverse-iteration over all nodes, starting from 'limit-1'
        let mut pos: EntryId = limit - 1;
        // 2. reverse-iteration over nodes of var, starting from 'root.p'
        // (presumably far away from limit)
        let mut prev: EntryId = self.entries[self.root_entries[&var]].p;
        while pos > 0 && self.entries[pos].v != var && prev > limit {
            pos -= 1;
            prev = self.entries[prev].p;
        }
        if prev > limit {
            pos
        } else {
            prev
        }
    }

    fn get_cause_at(&self, entry: EntryId) -> CId {
        self.entries[entry].cause.unwrap()
    }

    fn get_var_at(&self, entry: EntryId) -> VarRef {
        self.entries[entry].v
    }

    fn get_predecessor_of(&self, entry: EntryId) -> EntryId {
        self.entries[entry].p
    }
}

#[derive(Default)]
struct Front {
    inner: IdxHeap<VarRef, Reverse<EntryId>>,
}
impl Front {
    pub fn put(&mut self, v: VarRef, prio: EntryId) {
        if !self.inner.is_declared(v) {
            self.inner.declare_element(v, Reverse(prio));
            self.inner.enqueue(v);
        } else if !self.inner.is_enqueued(v) {
            self.inner.set_priority(v, Reverse(prio));
            self.inner.enqueue(v);
        } else {
            panic!()
        }
    }
    pub fn replace(&mut self, v: VarRef, new_prio: EntryId) {
        assert!(self.inner.is_declared(v) && self.inner.is_enqueued(v));
        self.inner.set_priority(v, Reverse(new_prio));
    }

    pub fn poll_last_value(&mut self) -> EntryId {
        let key = self.inner.pop().unwrap();
        let Reverse(value) = self.inner.priority(key);
        value
    }
    pub fn peek_last_value(&mut self) -> Option<EntryId> {
        match self.inner.peek() {
            Some(key) => {
                let Reverse(value) = self.inner.priority(*key);
                Some(value)
            }
            None => None,
        }
    }

    pub fn get_value(&self, key: VarRef) -> Option<EntryId> {
        if !self.inner.is_declared(key) || !self.inner.is_enqueued(key) {
            None
        } else {
            Some(self.inner.priority(key).0)
        }
    }
}

pub struct Explanation<'a> {
    /// conflicting nodes
    front: Front,
    /// literals that explain the conflict
    literals: RefSet<VarRef>,
    /// The decision to refute (ie, point to jump to wrt the current decision path).
    ///
    /// 0 represents the ROOT node,
    /// any value greater than the decision path is ignored,
    /// otherwise it represents the decision to refute in the decision path.
    assert_level: usize,
    /// the implication graph
    ig: LazyImplication<'a>,
}

impl Explanation<'_> {
    fn new<'csp>(implications: LazyImplication<'csp>) -> Explanation<'csp> {
        Explanation {
            front: Default::default(),
            literals: Default::default(),
            assert_level: 0,
            ig: implications,
        }
    }

    pub fn learn_clause_for_conflict(&mut self, cid: CId, var: VarRef) {
        // init front(cex)
        self.ig.collect_nodes_from_conflict(cid, var, &mut self.front);
        self.process();
    }

    fn process(&mut self) {
        let mut first = true;
        let mut current;
        while first || !self.stop() {
            first = false;
            current = self.front.poll_last_value();
            self.ig.predecessors_of(current, &mut self.front);
            let cause = self.ig.get_cause_at(current);
            self.explain(cause, current);
            self.relax();
        }
    }

    fn explain(&mut self, cid: CId, entry: EntryId) {
        todo!()
    }

    fn stop(&self) -> bool {
        todo!()
    }

    fn relax(&mut self) {
        // let mut k = usize::MAX;
        // while let Some(l) = self.front.peek_last_value() {
        //     if l == k {
        //         break;
        //     }
        //     let var = self.ig.get_var_at(l);
        //     // remove variable in 'front' but not in literals
        //     // achieved lazily by only evaluating the right-most one
        //     if !self.literals.contains(var) {
        //         self.front.poll_last_value();
        //     } else {
        //         let p = self.ig.get_predecessor_of(l);
        //
        //         // go left as long as the right-most variable in 'front' contradicts 'literals'
        //         if p <l && // to avoid going before "root"
        //
        //         }
        //
        //     }
        // }
        todo!()
    }
}
