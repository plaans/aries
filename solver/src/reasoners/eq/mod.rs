mod domain;

use crate::backtrack::{Backtrack, DecLvl, EventIndex, ObsTrailCursor, Trail};
use crate::core::literals::Watches;
use crate::core::state::{Cause, Domains, Explanation, InvalidUpdate};
use crate::core::{IntCst, Lit, SignedVar, UpperBound, VarRef};
use crate::model::{Label, Model};
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use crate::reif::ReifExpr;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

#[derive(Copy, Clone, Debug)]
struct OutEdge {
    succ: Node,
    label: Lit,
    active: Lit,
}

impl OutEdge {
    pub fn new(succ: Node, label: Lit, active: Lit) -> OutEdge {
        OutEdge { succ, label, active }
    }
}

#[derive(Copy, Clone, Debug)]
struct InEdge {
    pred: Node,
    label: Lit,
    active: Lit,
}

impl InEdge {
    pub fn new(pred: Node, label: Lit, active: Lit) -> InEdge {
        InEdge { pred, label, active }
    }
}

#[derive(Copy, Clone)]
struct DirEdge {
    src: Node,
    tgt: Node,
    label: Lit,
    active: Lit,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct Pair {
    a: Node,
    b: Node,
}
impl Pair {
    pub fn new(a: impl Into<Node>, b: impl Into<Node>) -> Pair {
        let a = a.into();
        let b = b.into();
        if a <= b {
            Pair { a, b }
        } else {
            Pair { a: b, b: a }
        }
    }
}

#[derive(Hash, Eq, PartialEq, Copy, Clone, Debug, Ord, PartialOrd)]
pub enum Node {
    Var(VarRef),
    Val(IntCst),
}

impl From<VarRef> for Node {
    fn from(v: VarRef) -> Self {
        Node::Var(v)
    }
}
impl From<IntCst> for Node {
    fn from(v: IntCst) -> Self {
        Node::Val(v)
    }
}

fn var_of(n: Node) -> VarRef {
    match n {
        Node::Var(v) => v,
        Node::Val(_) => VarRef::ZERO,
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct DirEdgeId {
    src: Node,
    tgt: Node,
}
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct DirEdgeLabel {
    label: Lit,
    active: Lit,
}

#[derive(Clone, Debug)]
enum Event {
    EdgePropagation { x: Node, y: Node, z: Node },
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum InferenceCause {
    EdgePropagation(EventIndex),
    DomUpper,
    DomLower,
    DomNeq,
    DomEq,
    DomSingleton,
}

impl From<InferenceCause> for u32 {
    fn from(value: InferenceCause) -> Self {
        use InferenceCause::*;
        match value {
            EdgePropagation(e) => 0u32 + (u32::from(e) << 1),
            DomUpper => 1u32 + (0u32 << 1),
            DomLower => 1u32 + (1u32 << 1),
            DomNeq => 1u32 + (2u32 << 1),
            DomEq => 1u32 + (3u32 << 1),
            DomSingleton => 1u32 + (4u32 << 1),
        }
    }
}

impl From<u32> for InferenceCause {
    fn from(value: u32) -> Self {
        use InferenceCause::*;
        let kind = value & 0x1;
        let payload = value >> 1;
        match kind {
            0 => EdgePropagation(EventIndex::from(payload)),
            1 => match payload {
                0 => DomUpper,
                1 => DomLower,
                2 => DomNeq,
                3 => DomEq,
                4 => DomSingleton,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Default)]
struct Graph {
    nodes: HashSet<Node>,
    nodes_ordered: Vec<Node>,
    domains: domain::Domains,
    succs: HashMap<Node, Vec<OutEdge>>,
    preds: HashMap<Node, Vec<InEdge>>,
    labels: HashMap<DirEdgeId, DirEdgeLabel>,
    watches: Watches<DirEdge>,
}

impl Graph {
    fn add_dir_edge(&mut self, src: Node, tgt: Node, label: Lit, active: Lit) {
        let de = DirEdge {
            src,
            tgt,
            label,
            active,
        };
        let succs = self.succs.entry(src).or_insert_with(|| Vec::with_capacity(32));
        succs.push(OutEdge::new(tgt, label, active));
        let preds = self.preds.entry(tgt).or_insert_with(|| Vec::with_capacity(32));
        preds.push(InEdge::new(src, label, active));
        self.watches.add_watch(de.clone(), label);
        self.watches.add_watch(de.clone(), !label);
        self.watches.add_watch(de, active);
        self.labels
            .insert(DirEdgeId { src, tgt }, DirEdgeLabel { label, active });
        if let (Node::Var(var), Node::Val(val)) = (src, tgt) {
            self.domains.add_value(var, val, label);
        }
    }

    pub fn add_node(&mut self, v: impl Into<Node>, model: &mut impl ReifyEq) {
        let v = v.into();
        if let Node::Var(_var) = v {
            let (lb, ub) = model.domain(v);
            for val in lb..=ub {
                self.add_node(val, model);
            }
        }
        if !self.nodes.contains(&v) {
            // add edges to all other nodes
            let nodes = self.nodes_ordered.clone(); // TODO: optimize
            for &other in &nodes {
                let label = model.reify_eq(v, other);
                // the out-edge is active if the presence of tgt implies the presence of v
                let out_active = model.n_presence_implication(other, v);

                self.add_dir_edge(v, other, label, out_active);

                let in_active = model.n_presence_implication(v, other);
                self.add_dir_edge(other, v, label, in_active);
            }
            self.nodes.insert(v);
            self.nodes_ordered.push(v);
        }
    }

    fn label(&self, src: impl Into<Node>, tgt: impl Into<Node>) -> Lit {
        let src = src.into();
        let tgt = tgt.into();
        let key = DirEdgeId { src, tgt };
        debug_assert!(self.labels.contains_key(&key), "Not label for {:?}", key);
        self.labels[&key].label
    }
    fn active(&self, src: impl Into<Node>, tgt: impl Into<Node>) -> Lit {
        let src = src.into();
        let tgt = tgt.into();
        let key = DirEdgeId { src, tgt };
        debug_assert!(self.labels.contains_key(&key), "Not label for {:?}", key);
        self.labels[&key].active
    }
}

/// Sets the label of XZ, recording XYZ as the explanation for this change.
fn set_edge_label(
    value: bool,
    x: Node,
    y: Node,
    z: Node,
    domains: &mut Domains,
    graph: &Graph,
    trail: &mut Trail<Event>,
) -> Result<bool, InvalidUpdate> {
    let label = graph.label(x, z);
    let label = if value { label } else { !label };

    match domains.value(label) {
        Some(true) => Ok(false),
        _ => {
            // there might be a change, record event source to be able to explain it
            let event = Event::EdgePropagation { x, y, z };
            let id = trail.push(event);

            let cause = Cause::inference(ReasonerId::Eq, InferenceCause::EdgePropagation(id));
            domains.set(label, cause)
        }
    }
}

type DomainEvent = crate::core::state::Event;

#[derive(Clone)]
pub struct EqTheory {
    graph: Graph,
    cursor: ObsTrailCursor<DomainEvent>,
    trail: Trail<Event>,
}

impl EqTheory {
    pub fn new() -> EqTheory {
        EqTheory {
            graph: Default::default(),
            cursor: Default::default(),
            trail: Default::default(),
        }
    }

    pub fn add_node(&mut self, v: VarRef, model: &mut impl ReifyEq) {
        self.graph.add_node(v, model)
    }

    fn propagate_edge_event(&mut self, event: Lit, domains: &mut Domains) -> Result<(), InvalidUpdate> {
        let mut in_to_check: Vec<Node> = Vec::with_capacity(64);
        let mut out_to_check: Vec<Node> = Vec::with_capacity(64);
        debug_assert!(domains.entails(event) || domains.entails(!event));

        for e in self.graph.watches.watches_on(event) {
            let src = e.src;
            let tgt = e.tgt;
            if !domains.entails(e.active) {
                continue;
            }
            if domains.entails(e.label) {
                // edge: SRC ===> TGT
                for out in &self.graph.succs[&tgt] {
                    if out.succ == src {
                        continue;
                    }
                    if !domains.entails(out.active) {
                        continue;
                    }
                    if domains.entails(out.label) {
                        // edge: TGT ===> SUCC, enforce SRC ===> SUCC
                        if set_edge_label(true, src, tgt, out.succ, domains, &self.graph, &mut self.trail)? {
                            out_to_check.push(out.succ);
                        }
                    } else if domains.entails(!out.label) {
                        // edge TGT =!=> SUCC, enforce SRC =!=> SUCC
                        if set_edge_label(false, src, tgt, out.succ, domains, &self.graph, &mut self.trail)? {
                            out_to_check.push(out.succ);
                        }
                    }
                }
                for inc in &self.graph.preds[&src] {
                    if inc.pred == tgt {
                        continue;
                    }
                    if !domains.entails(inc.active) {
                        continue;
                    }
                    if domains.entails(inc.label) {
                        // edge: PRED ==> SRC, enforce PRED ===> TGT
                        if set_edge_label(true, inc.pred, src, tgt, domains, &self.graph, &mut self.trail)? {
                            in_to_check.push(inc.pred);
                        }
                    } else if domains.entails(!inc.label) {
                        // edge PRED =!=> SRC, enforce PRED =!=> TGT
                        if set_edge_label(false, inc.pred, src, tgt, domains, &self.graph, &mut self.trail)? {
                            in_to_check.push(inc.pred);
                        }
                    }
                }
            } else if domains.entails(!e.label) {
                // edge: SRC =!=> TGT
                for out in &self.graph.succs[&tgt] {
                    if !domains.entails(out.active) {
                        continue;
                    }
                    if domains.entails(out.label) {
                        // edge: TGT ===> SUCC, enforce SRC =!=> SUCC
                        if set_edge_label(false, src, tgt, out.succ, domains, &self.graph, &mut self.trail)? {
                            out_to_check.push(out.succ);
                        }
                    }
                }
                for inc in &self.graph.preds[&src] {
                    if !domains.entails(inc.active) {
                        continue;
                    }
                    if domains.entails(inc.label) {
                        // edge: PRED ==> SRC, enforce PRED =!=> TGT
                        if set_edge_label(false, inc.pred, src, tgt, domains, &self.graph, &mut self.trail)? {
                            in_to_check.push(inc.pred);
                        }
                    }
                }
            }
            let y = tgt;
            // we have a bunch of `X -> Y` and `Y -> Z` edges that were updated, now we check if any `X -> Z` edge
            // need to be updated as result of this change in the `X -> Y -> Z` path

            // first let us preprocess the edges to only keep the ones that are active and get their labels
            let xys = in_to_check
                .iter()
                .filter_map(|x| {
                    let e = self.graph.labels[&DirEdgeId { src: *x, tgt: y }];
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
                    let e = self.graph.labels[&DirEdgeId { src: y, tgt: *z }];
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
                        debug_assert!(domains.entails(self.graph.active(x, z)));
                        if yz {
                            // got y ===> z, enforce x ===> z
                            set_edge_label(true, x, y, z, domains, &self.graph, &mut self.trail)?;
                        } else {
                            // got y =!=> z, enforce x =!=> z
                            set_edge_label(false, x, y, z, domains, &self.graph, &mut self.trail)?;
                        }
                    }
                } else {
                    // x =!=> y
                    for &(z, yz) in &yzs {
                        debug_assert!(domains.entails(self.graph.active(x, z)));
                        if yz {
                            // y ===> z, enforce x =!=> z
                            set_edge_label(false, x, y, z, domains, &self.graph, &mut self.trail)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn propagate_domain_event(
        &mut self,
        v: SignedVar,
        new_ub: IntCst,
        previous_ub: IntCst,
        domains: &mut Domains,
    ) -> Result<(), InvalidUpdate> {
        let new_literal = Lit::from_parts(v, UpperBound::ub(new_ub));
        for (var, value) in self.graph.domains.eq_watches(new_literal) {
            let cause = Cause::inference(ReasonerId::Eq, InferenceCause::DomEq);
            domains.set_lb(var, value, cause)?;
            domains.set_ub(var, value, cause)?;
        }
        for (var, value) in self.graph.domains.neq_watches(new_literal) {
            let cause = Cause::inference(ReasonerId::Eq, InferenceCause::DomNeq);
            if domains.lb(var) == value {
                domains.set_lb(var, value + 1, cause)?;
            } else if domains.ub(var) == value {
                domains.set_ub(var, value - 1, cause)?;
            }
        }
        if self.graph.domains.has_domain(v.variable()) {
            for &invalid in self.graph.domains.values(v, new_ub + 1, previous_ub) {
                let cause = if v.is_plus() {
                    Cause::inference(ReasonerId::Eq, InferenceCause::DomUpper)
                } else {
                    Cause::inference(ReasonerId::Eq, InferenceCause::DomLower)
                };
                domains.set(!invalid, cause)?;
            }
            let mut updated_ub = new_ub;
            loop {
                let l = self.graph.domains.value(v, updated_ub);
                if domains.entails(!l) {
                    updated_ub -= 1;
                } else {
                    break;
                }
            }
            if updated_ub < new_ub {
                let cause = Cause::inference(ReasonerId::Eq, InferenceCause::DomNeq);
                domains.set(Lit::from_parts(v, UpperBound::ub(updated_ub)), cause)?;
            }
            let v = v.variable();
            if domains.lb(v) == domains.ub(v) {
                let cause = Cause::inference(ReasonerId::Eq, InferenceCause::DomSingleton);
                let l = self.graph.domains.value(SignedVar::plus(v), domains.lb(v));
                domains.set(l, cause)?;
            }
        }
        Ok(())
    }

    pub fn add_edge(&mut self, a: impl Into<Node>, b: impl Into<Node>, model: &mut impl ReifyEq) -> Lit {
        let a = a.into();
        let b = b.into();
        self.graph.add_node(a, model);
        self.graph.add_node(b, model);
        self.graph.label(a, b)
    }
}

impl Backtrack for EqTheory {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.trail.restore_last_with(|_| {})
    }
}

impl Theory for EqTheory {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Eq
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        let mut cursor_copy = self.cursor.clone();
        loop {
            let mut new_event_treated = false;

            while let Some(ev) = self.cursor.pop(domains.trail()) {
                if let Some(inference) = ev.cause.as_external_inference() {
                    if inference.writer == self.identity() {
                        continue; // already handled during propagation
                    }
                };

                self.propagate_edge_event(ev.new_literal(), domains)?;
                new_event_treated = true;
            }

            while let Some(ev) = cursor_copy.pop(domains.trail()).copied() {
                self.propagate_domain_event(
                    ev.affected_bound,
                    ev.new_value.as_int(),
                    ev.previous.value.as_int(),
                    domains,
                )?;
                new_event_treated = true;
            }

            if !new_event_treated {
                break;
            }
        }

        Ok(())
    }

    fn explain(&mut self, _: Lit, context: u32, domains: &Domains, out_explanation: &mut Explanation) {
        let cause = InferenceCause::from(context);
        match cause {
            InferenceCause::EdgePropagation(event_index) => {
                let event = self.trail.get_event(event_index);
                let &Event::EdgePropagation { x, y, z } = event;

                let mut push_causes = |a, b| {
                    let ab_act = self.graph.active(a, b);
                    debug_assert!(domains.entails(ab_act), "Propagation occurred on inactive edges.");
                    out_explanation.push(ab_act);
                    let ab_lbl = self.graph.label(a, b);
                    match domains.value(ab_lbl) {
                        Some(true) => out_explanation.push(ab_lbl),
                        Some(false) => out_explanation.push(!ab_lbl),
                        None => {
                            panic!("Propagation from unset labels")
                        }
                    }
                };
                push_causes(x, y);
                push_causes(y, z);
            }
            _ => todo!(),
            // InferenceCause::DomUpper => {}
            // InferenceCause::DomLower => {}
            // InferenceCause::DomNeq => {}
            // InferenceCause::DomEq => {}
            // InferenceCause::DomSingleton => {}
        }
    }

    fn print_stats(&self) {
        // TODO
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

pub trait ReifyEq {
    fn domain(&self, a: Node) -> (IntCst, IntCst);
    fn reify_eq(&mut self, a: Node, b: Node) -> Lit;

    /// Return a literal that is true iff p(a) => p(b)
    fn presence_implication(&self, a: VarRef, b: VarRef) -> Lit;

    fn n_presence_implication(&self, a: Node, b: Node) -> Lit {
        self.presence_implication(var_of(a), var_of(b))
    }
}

impl<L: Label> ReifyEq for Model<L> {
    fn reify_eq(&mut self, a: Node, b: Node) -> Lit {
        use Node::*;
        match (a, b) {
            (Var(a), Var(b)) => {
                let e = if a < b { ReifExpr::Eq(a, b) } else { ReifExpr::Eq(b, a) };
                self.reify_core(e, false)
            }
            (Var(a), Val(b)) | (Val(b), Var(a)) => {
                let e = ReifExpr::EqVal(a, b);
                self.reify_core(e, false)
            }
            (Val(a), Val(b)) => {
                if a == b {
                    Lit::TRUE
                } else {
                    Lit::FALSE
                }
            }
        }
    }

    fn presence_implication(&self, a: VarRef, b: VarRef) -> Lit {
        let pa = self.state.presence(a);
        let pb = self.state.presence(b);
        if self.state.implies(pa, pb) {
            Lit::TRUE
        } else {
            pb
        }
    }

    fn domain(&self, a: Node) -> (IntCst, IntCst) {
        match a {
            Node::Var(v) => self.state.bounds(v),
            Node::Val(v) => (v, v),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::backtrack::{Backtrack, EventIndex};
    use crate::core::state::{Cause, Domains, SingleTheoryExplainer};
    use crate::core::{IntCst, Lit, VarRef};
    use crate::model::lang::expr::eq;
    use crate::model::symbols::SymbolTable;
    use crate::model::types::TypeHierarchy;
    use crate::model::Model;
    use crate::reasoners::eq::{EqTheory, InferenceCause, Node, Pair, ReifyEq};
    use crate::reasoners::{Contradiction, Theory};
    use crate::solver::Solver;
    use crate::utils::input::Sym;
    use itertools::Itertools;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

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
                for v in 0..=10 {
                    let key = Pair::new(vars[i], Node::Val(v));
                    map.insert(key, domains.new_var(0, 1).geq(1));
                }
            }
            Eqs { map }
        }

        fn get(&self, a: impl Into<Node>, b: impl Into<Node>) -> Lit {
            let a = a.into();
            let b = b.into();
            use Node::*;
            match (a, b) {
                _ if a == b => Lit::TRUE,
                (Val(a), Val(b)) => {
                    assert_ne!(a, b);
                    Lit::FALSE
                }
                _ => *self
                    .map
                    .get(&Pair::new(a, b))
                    .unwrap_or_else(|| panic!("No entry for key ({a:?} {b:?}")),
            }
        }
    }

    impl ReifyEq for Eqs {
        fn reify_eq(&mut self, a: Node, b: Node) -> Lit {
            self.get(a, b)
        }

        fn presence_implication(&self, _a: VarRef, _b: VarRef) -> Lit {
            Lit::TRUE // Only correct for non optional variables
        }

        fn domain(&self, a: Node) -> (IntCst, IntCst) {
            match a {
                Node::Var(_) => (0, 10), // not general
                Node::Val(v) => (v, v),
            }
        }
    }

    #[test]
    fn test_manual_propagation() {
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

    /// Tests the propagation and explanation through the theory binding, reacting to events in the domains.
    #[test]
    fn test_automated_propagation() {
        let domains = &mut Domains::new();

        let a = domains.new_var(0, 10);
        let b = domains.new_var(0, 10);
        let c = domains.new_var(0, 10);

        let vars = [a, b, c];

        let eqs = &mut Eqs::init(&vars, domains);
        let mut theory = EqTheory::new();
        let ab = theory.add_edge(a, b, eqs);
        let bc = theory.add_edge(b, c, eqs);
        let ac = theory.add_edge(a, c, eqs);

        domains.save_state();
        theory.save_state();
        domains.set(ab, Cause::Decision).expect("Invalid decision");
        theory.propagate(domains).unwrap();

        domains.save_state();
        theory.save_state();
        domains.set(bc, Cause::Decision).expect("Invalid decision");
        theory.propagate(domains).unwrap();
        assert_eq!(domains.value(ac), Some(true));
        domains.restore_last();
        theory.restore_last();

        domains.save_state();
        theory.save_state();
        domains.set(!bc, Cause::Decision).expect("Invalid decision");
        theory.propagate(domains).unwrap();

        assert_eq!(domains.value(!ac), Some(true));
        domains.restore_last();
        theory.restore_last();

        domains.save_state();
        theory.save_state();
        domains.set(ac, Cause::Decision).expect("Invalid decision");
        domains.save_state();
        theory.save_state();
        domains.set(!bc, Cause::Decision).expect("Invalid decision");
        let Err(contradiction) = theory.propagate(domains) else {
            panic!("Undetected inconsistency")
        };
        let explainer = &mut SingleTheoryExplainer(&mut theory);
        let clause = match contradiction {
            Contradiction::InvalidUpdate(up) => domains.clause_for_invalid_update(up, explainer),
            Contradiction::Explanation(expl) => domains.refine_explanation(expl, explainer),
        };
        println!("ab: {ab:?}, bc: {bc:?}, ac: {ac:?}");
        println!("Clause: {:?}", clause);
        assert_eq!(
            HashSet::from_iter(clause.clause.into_iter()),
            HashSet::from([!ab, bc, !ac]) // ab & !bc => !ac
        );
    }

    type S = &'static str;
    impl From<Vec<(S, Vec<S>)>> for SymbolTable {
        fn from(value: Vec<(S, Vec<S>)>) -> Self {
            let types = value.iter().map(|e| (Sym::new(e.0), None)).collect_vec();
            let types = TypeHierarchy::new(types).unwrap();
            let mut instances = Vec::new();
            for tpe in value {
                for instance in tpe.1 {
                    instances.push((Sym::from(instance), Sym::from(tpe.0)))
                }
            }
            SymbolTable::new(types, instances).unwrap()
        }
    }

    #[test]
    fn test_model() {
        let symbols = SymbolTable::from(vec![("obj", vec!["a", "b", "c"])]);
        let symbols = Arc::new(symbols);

        let obj = symbols.types.id_of("obj").unwrap();

        let mut model: Model<S> = Model::new_with_symbols(symbols.clone());
        let x = model.new_sym_var(obj, "X");
        let y = model.new_sym_var(obj, "Y");
        let _z = model.new_sym_var(obj, "Z");

        let _xy = model.reify(eq(x, y));

        let solver = &mut Solver::new(model);
        solver.solve().unwrap();
    }

    #[test]
    fn test_inference_cause_conversion() {
        let serde = |c: InferenceCause| {
            let serialized: u32 = c.into();
            InferenceCause::from(serialized)
        };
        let tests = [
            InferenceCause::EdgePropagation(EventIndex::new(4)),
            InferenceCause::DomSingleton,
            InferenceCause::DomUpper,
            InferenceCause::DomLower,
            InferenceCause::DomEq,
            InferenceCause::DomNeq,
        ];
        for t in tests {
            assert_eq!(t, serde(t));
        }
    }
}
