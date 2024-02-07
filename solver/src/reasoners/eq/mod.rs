use crate::collections::ref_store::RefMap;
use crate::core::literals::Watches;
use crate::core::state::{Cause, Domains, InvalidUpdate};
use crate::core::{Lit, VarRef};
use crate::reasoners::ReasonerId;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

struct OutEdge {
    succ: VarRef,
    label: Lit,
    active: Lit,
}

impl OutEdge {
    pub fn new(succ: VarRef, label: Lit, active: Lit) -> OutEdge {
        OutEdge { succ, label, active }
    }
}

struct InEdge {
    pred: VarRef,
    label: Lit,
    active: Lit,
}

impl InEdge {
    pub fn new(pred: VarRef, label: Lit, active: Lit) -> InEdge {
        InEdge { pred, label, active }
    }
}

trait ReifyEq {
    fn reify_eq(&mut self, a: VarRef, b: VarRef) -> Lit;

    /// Return a literal that is true iff p(a) => p(b)
    fn presence_implication(&self, a: VarRef, b: VarRef) -> Lit;
}

#[derive(Copy, Clone)]
struct DirEdge {
    src: VarRef,
    tgt: VarRef,
    label: Lit,
    active: Lit,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct Pair {
    a: VarRef,
    b: VarRef,
}
impl Pair {
    pub fn new(a: VarRef, b: VarRef) -> Pair {
        if a <= b {
            Pair { a, b }
        } else {
            Pair { a: b, b: a }
        }
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct DirEdgeId {
    src: VarRef,
    tgt: VarRef,
}
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct DirEdgeLabel {
    label: Lit,
    active: Lit,
}

pub struct EqTheory {
    nodes: HashSet<VarRef>,
    succs: RefMap<VarRef, Vec<OutEdge>>,
    preds: RefMap<VarRef, Vec<InEdge>>,
    labels: HashMap<DirEdgeId, DirEdgeLabel>,
    watches: Watches<DirEdge>,
}

impl EqTheory {
    pub fn new() -> EqTheory {
        EqTheory {
            nodes: Default::default(),
            succs: Default::default(),
            preds: Default::default(),
            labels: Default::default(),
            watches: Default::default(),
        }
    }
    fn add_dir_edge(&mut self, src: VarRef, tgt: VarRef, label: Lit, active: Lit) {
        let de = DirEdge {
            src,
            tgt,
            label,
            active,
        };
        let succs = self.succs.get_mut_or_insert(src, || Vec::with_capacity(32));
        succs.push(OutEdge::new(tgt, label, active));
        let preds = self.preds.get_mut_or_insert(tgt, || Vec::with_capacity(32));
        preds.push(InEdge::new(src, label, active));
        self.watches.add_watch(de.clone(), label);
        self.watches.add_watch(de.clone(), !label);
        self.watches.add_watch(de, active);
        self.labels
            .insert(DirEdgeId { src, tgt }, DirEdgeLabel { label, active });
    }

    pub fn add_node(&mut self, v: VarRef, model: &mut impl ReifyEq) {
        if !self.nodes.contains(&v) {
            // add edges to all other nodes
            let nodes = self.nodes.clone(); // TODO: optimize
            for &other in &nodes {
                let label = model.reify_eq(v, other);
                // the out-edge is active if the presence of tgt implies the presence of v
                let out_active = model.presence_implication(other, v);
                self.add_dir_edge(v, other, label, out_active);

                let in_active = model.presence_implication(v, other);
                self.add_dir_edge(other, v, label, in_active);
            }
            self.nodes.insert(v);
        }
    }

    fn label(&self, src: VarRef, tgt: VarRef) -> Lit {
        let key = DirEdgeId { src, tgt };
        debug_assert!(self.labels.contains_key(&key), "Not label for {:?}", key);
        self.labels[&key].label
    }
    fn active(&self, src: VarRef, tgt: VarRef) -> Lit {
        let key = DirEdgeId { src, tgt };
        debug_assert!(self.labels.contains_key(&key), "Not label for {:?}", key);
        self.labels[&key].active
    }
    fn propagate_edge_event(&mut self, event: Lit, domains: &mut Domains) -> Result<(), InvalidUpdate> {
        let cause = Cause::inference(ReasonerId::Eq, 0u32);
        let mut in_to_check: Vec<VarRef> = Vec::with_capacity(64);
        let mut out_to_check: Vec<VarRef> = Vec::with_capacity(64);
        for e in self.watches.watches_on(event) {
            let src = e.src;
            let tgt = e.tgt;
            if !domains.entails(e.active) {
                continue;
            }
            if domains.entails(e.label) {
                // edge: SRC ===> TGT
                for out in &self.succs[tgt] {
                    if out.succ == src {
                        continue;
                    }
                    if !domains.entails(out.active) {
                        continue;
                    }
                    if domains.entails(out.label) {
                        // edge: TGT ===> SUCC, enforce SRC ===> SUCC
                        if domains.set(self.label(src, out.succ), cause)? {
                            out_to_check.push(out.succ);
                        }
                    } else if domains.entails(!out.label) {
                        // edge TGT =!=> SUCC, enforce SRC =!=> SUCC
                        if domains.set(!self.label(src, out.succ), cause)? {
                            out_to_check.push(out.succ);
                        }
                    }
                }
                for inc in &self.preds[src] {
                    if inc.pred == tgt {
                        continue;
                    }
                    if !domains.entails(inc.active) {
                        continue;
                    }
                    if domains.entails(inc.label) {
                        // edge: PRED ==> SRC, enforce PRED ===> TGT
                        if domains.set(self.label(inc.pred, tgt), cause)? {
                            in_to_check.push(inc.pred);
                        }
                    } else if domains.entails(!inc.label) {
                        // edge PRED =!=> SRC, enforce PRED =!=> TGT
                        if domains.set(!self.label(inc.pred, tgt), cause)? {
                            in_to_check.push(inc.pred);
                        }
                    }
                }
            } else if domains.entails(!e.label) {
                // edge: SRC =!=> TGT
                for out in &self.succs[tgt] {
                    if !domains.entails(out.active) {
                        continue;
                    }
                    if domains.entails(out.label) {
                        // edge: TGT ===> SUCC, enforce SRC =!=> SUCC
                        if domains.set(!self.label(src, out.succ), cause)? {
                            out_to_check.push(out.succ);
                        }
                    }
                }
                for inc in &self.preds[src] {
                    if !domains.entails(inc.active) {
                        continue;
                    }
                    if domains.entails(inc.label) {
                        // edge: PRED ==> SRC, enforce PRED =!=> TGT
                        if domains.set(!self.label(inc.pred, tgt), cause)? {
                            in_to_check.push(inc.pred);
                        }
                    }
                }
            }
            let y = tgt;
            let xys = in_to_check
                .iter()
                .filter_map(|x| {
                    let e = self.labels[&DirEdgeId { src: *x, tgt: y }];
                    if !domains.entails(e.active) {
                        return None;
                    }
                    let Some(label) = domains.value(e.label) else {
                        return None;
                    };
                    Some((*x, label))
                })
                .collect_vec();
            let yzs = out_to_check
                .iter()
                .filter_map(|z| {
                    let e = self.labels[&DirEdgeId { src: y, tgt: *z }];
                    if !domains.entails(e.active) {
                        return None;
                    }
                    let Some(label) = domains.value(e.label) else {
                        return None;
                    };
                    Some((*z, label))
                })
                .collect_vec();

            for &(x, xy) in &xys {
                if xy {
                    // x ===> y
                    for &(z, yz) in &yzs {
                        debug_assert!(domains.entails(self.active(x, z)));
                        if yz {
                            // got y ===> z, enforce x ===> z
                            domains.set(self.label(x, z), cause)?;
                        } else {
                            // got y =!=> z, enforce x =!=> z
                            domains.set(!self.label(x, z), cause)?;
                        }
                    }
                } else {
                    // x =!=> y
                    for &(z, yz) in &yzs {
                        debug_assert!(domains.entails(self.active(x, z)));
                        if yz {
                            // y ===> z, enforce x =!=> z
                            domains.set(!self.label(x, z), cause)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn add_edge(&mut self, a: VarRef, b: VarRef, model: &mut impl ReifyEq) -> Lit {
        self.add_node(a, model);
        self.add_node(b, model);
        self.label(a, b)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::state::{Cause, Domains};
    use crate::core::{Lit, VarRef};
    use crate::reasoners::eq::{EqTheory, Pair, ReifyEq};
    use std::collections::HashMap;

    struct Eqs {
        map: HashMap<Pair, Lit>,
    }
    impl Eqs {
        pub fn init(vars: &[VarRef], domains: &mut Domains) -> Eqs {
            let mut map = HashMap::new();
            for i in 0..vars.len() {
                for j in (i + 1)..vars.len() {
                    let key = Pair::new(vars[i], vars[j]);
                    map.insert(key, domains.new_var(0, 1).geq(1));
                }
            }
            Eqs { map }
        }

        fn get(&self, a: VarRef, b: VarRef) -> Lit {
            if a == b {
                Lit::TRUE
            } else {
                self.map[&Pair::new(a, b)]
            }
        }
    }

    impl ReifyEq for Eqs {
        fn reify_eq(&mut self, a: VarRef, b: VarRef) -> Lit {
            self.get(a, b)
        }

        fn presence_implication(&self, _a: VarRef, _b: VarRef) -> Lit {
            Lit::TRUE // Only correct for non optional variables
        }
    }

    #[test]
    fn test() {
        let domains = &mut Domains::new();

        let x = domains.new_var(0, 10);
        let a = domains.new_var(0, 10);
        let b = domains.new_var(0, 10);
        let b1 = domains.new_var(0, 10);
        let b2 = domains.new_var(0, 10);
        let d = domains.new_var(0, 10);
        let d2 = domains.new_var(0, 10);
        let vars = [x, a, b, b1, b2, d, d2];

        let mut eqs = Eqs::init(&vars, domains);
        let mut theory = EqTheory::new();
        for v in &vars {
            theory.add_node(*v, &mut eqs)
        }

        let mut set = |label, domains: &mut Domains| {
            domains.set(label, Cause::Decision).expect("Decision error");
            theory.propagate_edge_event(label, domains).expect("Propagation error");
        };
        let check = |eq1: &[VarRef], eq2: &[VarRef], domains: &Domains| {
            println!("Check {:?}  !=  {:?}", eq1, eq2);
            for &x in eq1 {
                for &y in eq1 {
                    assert_eq!(domains.value(eqs.get(x, y)), Some(true), "{x:?} = {y:?}");
                }

                for &z in eq2 {
                    assert_eq!(domains.value(eqs.get(x, z)), Some(false), "{x:?} = {z:?}");
                }
            }
            for &x in eq2 {
                for &y in eq2 {
                    assert_eq!(domains.value(eqs.get(x, y)), Some(true), "{x:?} = {y:?}");
                }
            }
        };
        set(eqs.get(x, a), domains);
        set(eqs.get(b2, b1), domains);
        set(eqs.get(d2, d), domains);
        set(eqs.get(b, b1), domains);
        set(!eqs.get(b, d), domains);

        assert_eq!(domains.value(eqs.get(b, b1)), Some(true));
        assert_eq!(domains.value(eqs.get(b, b2)), Some(true));
        assert_eq!(domains.value(eqs.get(b, d)), Some(false));
        assert_eq!(domains.value(eqs.get(b, d2)), Some(false));
        assert_eq!(domains.value(eqs.get(a, b)), None);
        check(&[b, b1, b2], &[d, d2], domains); // more systematic

        set(eqs.get(a, b), domains);
        check(&[a, b, b1, b2, x], &[d, d2], domains);
    }
}
