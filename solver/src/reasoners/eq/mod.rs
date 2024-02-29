mod domain;

use crate::backtrack::{Backtrack, DecLvl, EventIndex, ObsTrailCursor, Trail};
use crate::core::literals::Watches;
use crate::core::state::{Cause, Domains, Explanation, InvalidUpdate};
use crate::core::{IntCst, Lit, SignedVar, UpperBound, VarRef, INT_CST_MAX, INT_CST_MIN};
use crate::model::{Label, Model};
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use crate::reif::ReifExpr;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

#[derive(Copy, Clone, Debug, PartialEq)]
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

#[derive(Copy, Clone, Debug)]
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
    EdgeEnabledPos(DirEdge),
    EdgeEnabledNeg(DirEdge),
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
    succs_pos: HashMap<Node, Vec<OutEdge>>,
    preds_pos: HashMap<Node, Vec<InEdge>>,
    succs_neg: HashMap<Node, Vec<OutEdge>>,
    preds_neg: HashMap<Node, Vec<InEdge>>,
    labels: HashMap<DirEdgeId, DirEdgeLabel>,
    watches: Watches<DirEdge>,
}

impl Graph {
    fn add_dir_edge(&mut self, src: Node, tgt: Node, label: Lit, active: Lit, model: &mut impl ReifyEq) {
        let de = DirEdge {
            src,
            tgt,
            label,
            active,
        };

        self.watches.add_watch(de.clone(), label);
        self.watches.add_watch(de.clone(), !label);
        self.watches.add_watch(de, active);
        self.labels
            .insert(DirEdgeId { src, tgt }, DirEdgeLabel { label, active });
        if let (Node::Var(var), Node::Val(val)) = (src, tgt) {
            let (lb, ub) = model.domain(Node::Var(var));
            if (lb..=ub).contains(&val) {
                self.domains.add_value(var, val, label);
            }
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
            self.succs_pos.insert(v, Vec::new());
            self.preds_pos.insert(v, Vec::new());
            self.succs_neg.insert(v, Vec::new());
            self.preds_neg.insert(v, Vec::new());

            // add edges to all other nodes
            let nodes = self.nodes_ordered.iter().copied().sorted().collect_vec(); // TODO: optimize
            for &other in &nodes {
                let label = model.reify_eq(v, other);
                // the out-edge is active if the presence of tgt implies the presence of v
                let out_active = model.n_presence_implication(other, v);
                self.add_dir_edge(v, other, label, out_active, model);

                let in_active = model.n_presence_implication(v, other);
                self.add_dir_edge(other, v, label, in_active, model);
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

    debug_assert!(domains.entails(graph.active(x, z)), "xz not active when xy and yz are");

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

#[derive(Clone, Default)]
struct Stats {
    num_edge_propagations: usize,
    num_edge_propagations_pos: usize,
    num_edge_propagations_neg: usize,
    num_edge_propagation1_pos_pos: usize,
    num_edge_propagation1_pos_neg: usize,
    num_edge_propagation1_neg_pos: usize,
    num_edge_propagation1_effective: usize,
    num_edge_propagation2_pos_pos: usize,
    num_edge_propagation2_pos_neg: usize,
    num_edge_propagation2_neg_pos: usize,
}

#[derive(Clone)]
pub struct EqTheory {
    graph: Graph,
    cursor: ObsTrailCursor<DomainEvent>,
    trail: Trail<Event>,
    stats: Stats,
}

impl EqTheory {
    pub fn new() -> EqTheory {
        EqTheory {
            graph: Default::default(),
            cursor: Default::default(),
            trail: Default::default(),
            stats: Default::default(),
        }
    }

    pub fn add_node(&mut self, v: VarRef, model: &mut impl ReifyEq) {
        self.graph.add_node(v, model)
    }

    /// If this event enables an edge, then add it to the graph
    /// TODO: make sure the edge is only added once
    fn update_graph(&mut self, event: Lit, domains: &mut Domains) {
        for e in self.graph.watches.watches_on(event) {
            if !domains.entails(e.active) || domains.value(e.label).is_none() {
                continue;
            }
            match domains.value(e.label) {
                Some(true) => {
                    if self.graph.succs_pos[&e.src]
                        .iter()
                        .find(|ee| ee.succ == e.tgt)
                        .is_some()
                    {
                        continue; // edge already present, skip
                    }
                    self.graph
                        .succs_pos
                        .get_mut(&e.src)
                        .unwrap()
                        .push(OutEdge::new(e.tgt, e.label, e.active));
                    self.graph
                        .preds_pos
                        .get_mut(&e.tgt)
                        .unwrap()
                        .push(InEdge::new(e.src, e.label, e.active));
                    self.trail.push(Event::EdgeEnabledPos(e));
                }
                Some(false) => {
                    if self.graph.succs_neg[&e.src]
                        .iter()
                        .find(|ee| ee.succ == e.tgt)
                        .is_some()
                    {
                        continue; // edge already present, skip
                    }
                    // debug_assert_eq!(self.graph.succs_neg[&e.src].iter().find(|ee| ee.succ == e.tgt), None);
                    self.graph
                        .succs_neg
                        .get_mut(&e.src)
                        .unwrap()
                        .push(OutEdge::new(e.tgt, e.label, e.active));
                    self.graph
                        .preds_neg
                        .get_mut(&e.tgt)
                        .unwrap()
                        .push(InEdge::new(e.src, e.label, e.active));
                    self.trail.push(Event::EdgeEnabledNeg(e));
                }
                None => {} // edge not enabled
            }
        }
    }

    fn propagate_edge_event(&mut self, event: Lit, domains: &mut Domains) -> Result<(), InvalidUpdate> {
        let mut in_to_check: Vec<Node> = Vec::with_capacity(64);
        let mut out_to_check: Vec<Node> = Vec::with_capacity(64);
        debug_assert!(domains.entails(event));
        // TODO: this one may be propagated twice (one per watch)
        for e in self.graph.watches.watches_on(event) {
            in_to_check.clear();
            out_to_check.clear();
            self.stats.num_edge_propagations += 1;
            let src = e.src;
            let tgt = e.tgt;
            if !domains.entails(e.active) || domains.value(e.label).is_none() {
                continue; // edge is not enabled
            }

            if domains.entails(e.label) {
                self.stats.num_edge_propagations_pos += 1;
                // edge: SRC ===> TGT
                for out in &self.graph.succs_pos[&tgt] {
                    if out.succ == src {
                        continue;
                    }
                    // edge: TGT ===> SUCC, enforce SRC ===> SUCC
                    self.stats.num_edge_propagation1_pos_pos += 1;
                    debug_assert!(domains.entails(out.active));
                    debug_assert!(domains.entails(out.label));
                    if set_edge_label(true, src, tgt, out.succ, domains, &self.graph, &mut self.trail)? {
                        out_to_check.push(out.succ);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                }
                for out in &self.graph.succs_neg[&tgt] {
                    if out.succ == src {
                        continue;
                    }
                    // edge TGT =!=> SUCC, enforce SRC =!=> SUCC
                    self.stats.num_edge_propagation1_pos_neg += 1;
                    debug_assert!(domains.entails(out.active));
                    debug_assert!(domains.entails(!out.label));
                    if set_edge_label(false, src, tgt, out.succ, domains, &self.graph, &mut self.trail)? {
                        out_to_check.push(out.succ);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                }
                for inc in &self.graph.preds_pos[&src] {
                    if inc.pred == tgt {
                        continue;
                    }
                    self.stats.num_edge_propagation1_pos_pos += 1;
                    debug_assert!(domains.entails(inc.active));

                    // println!("  +in {inc:?}");
                    debug_assert!(domains.entails(inc.label));
                    // edge: PRED ==> SRC, enforce PRED ===> TGT
                    if set_edge_label(true, inc.pred, src, tgt, domains, &self.graph, &mut self.trail)? {
                        in_to_check.push(inc.pred);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                }
                for inc in &self.graph.preds_neg[&src] {
                    if inc.pred == tgt {
                        continue;
                    }
                    self.stats.num_edge_propagation1_pos_neg += 1;
                    debug_assert!(domains.entails(inc.active));

                    // println!("  +in {inc:?}");
                    debug_assert!(!domains.entails(inc.label));
                    // edge: PRED =!> SRC, enforce PRED =!=> TGT
                    if set_edge_label(false, inc.pred, src, tgt, domains, &self.graph, &mut self.trail)? {
                        in_to_check.push(inc.pred);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                }
            } else {
                debug_assert!(domains.entails(!e.label));
                self.stats.num_edge_propagations_neg += 1;
                // edge: SRC =!=> TGT
                for out in &self.graph.succs_pos[&tgt] {
                    self.stats.num_edge_propagation1_neg_pos += 1;
                    debug_assert!(domains.entails(out.active));
                    debug_assert!(domains.entails(out.label));
                    // edge: TGT ===> SUCC, enforce SRC =!=> SUCC
                    if set_edge_label(false, src, tgt, out.succ, domains, &self.graph, &mut self.trail)? {
                        out_to_check.push(out.succ);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                }
                for inc in &self.graph.preds_pos[&src] {
                    self.stats.num_edge_propagation1_neg_pos += 1;
                    debug_assert!(domains.entails(inc.active));
                    debug_assert!(domains.entails(inc.label));
                    // edge: PRED ==> SRC, enforce PRED =!=> TGT
                    if set_edge_label(false, inc.pred, src, tgt, domains, &self.graph, &mut self.trail)? {
                        in_to_check.push(inc.pred);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                }
            }
            let y = tgt;
            // we have a bunch of `X -> Y` and `Y -> Z` edges that were updated, now we check if any `X -> Z` edge
            // need to be updated as result of this change in the `X -> Y -> Z` path

            // first let us preprocess the edges to only keep the ones that are active and get their labels
            let mut xys_pos = Vec::with_capacity(64);
            let mut xys_neg = Vec::with_capacity(64);
            for &x in &in_to_check {
                if x == y {
                    continue;
                }
                let e = self.graph.labels[&DirEdgeId { src: x, tgt: y }];
                debug_assert!(domains.entails(e.active));
                match domains.value(e.label) {
                    Some(true) => xys_pos.push(x),
                    Some(false) => xys_neg.push(x),
                    None => {}
                }
            }
            let mut yzs_pos = Vec::with_capacity(64);
            let mut yzs_neg = Vec::with_capacity(64);
            for &z in &out_to_check {
                if y == z {
                    continue;
                }
                let e = self.graph.labels[&DirEdgeId { src: y, tgt: z }];
                debug_assert!(domains.entails(e.active));
                match domains.value(e.label) {
                    Some(true) => yzs_pos.push(z),
                    Some(false) => yzs_neg.push(z),
                    None => {}
                }
            }
            for &x in &xys_pos {
                // x ===> y
                for &z in &yzs_pos {
                    if x == z {
                        continue;
                    }
                    self.stats.num_edge_propagation2_pos_pos += 1;
                    set_edge_label(true, x, y, z, domains, &self.graph, &mut self.trail)?;
                }
                for &z in &yzs_neg {
                    if x == z {
                        continue;
                    }
                    self.stats.num_edge_propagation2_pos_neg += 1;
                    set_edge_label(false, x, y, z, domains, &self.graph, &mut self.trail)?;
                }
            }

            for x in xys_neg {
                // x =!=> y
                for &z in &yzs_pos {
                    if x == z {
                        continue;
                    }
                    set_edge_label(false, x, y, z, domains, &self.graph, &mut self.trail)?;
                    self.stats.num_edge_propagation2_neg_pos += 1;
                }
            }
        }
        Ok(())
    }

    /// DomEq : (x = v) => (x >= v)
    ///         (x = v) => (x <= v)
    /// DomNeq: (x != v) & (x <= v) => (x <= v - 1)
    ///         (x != v) & (x >= v) => (x >= v + 1)
    ///         (x != v) & (-x <= -v) => (-x <= -v-1)  (rewrite of previous for uniformity with signed vars
    ///           
    /// DomUpper: (x <= v) => (x != v+1)  
    /// DomLower: (x >= v) => (x != v-1)
    /// DomSingleton: (x >= v) & (x <= v) => (x = v)  
    pub fn propagate_domain_event(
        &mut self,
        v: SignedVar,
        new_ub: IntCst,
        previous_ub: IntCst,
        domains: &mut Domains,
    ) -> Result<(), InvalidUpdate> {
        let new_literal = Lit::from_parts(v, UpperBound::ub(new_ub));
        for (var, value) in self.graph.domains.eq_watches(new_literal) {
            let val_rep = self.graph.domains.value(var, value);
            debug_assert_eq!(val_rep, Some(new_literal));
            debug_assert!(domains.entails(new_literal));
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
                    // dbg!(invalid, v, new_ub + 1, previous_ub);
                    Cause::inference(ReasonerId::Eq, InferenceCause::DomLower)
                };
                domains.set(!invalid, cause)?;
            }

            // reduce domain if the upper bound is excluded
            let mut updated_ub = new_ub;
            while let Some(l) = self.graph.domains.signed_value(v, updated_ub) {
                if domains.entails(!l) {
                    updated_ub -= 1;
                    let cause = Cause::inference(ReasonerId::Eq, InferenceCause::DomNeq);
                    domains.set(Lit::from_parts(v, UpperBound::ub(updated_ub)), cause)?;
                } else {
                    break;
                }
            }

            let v = v.variable();
            if domains.lb(v) == domains.ub(v) {
                let cause = Cause::inference(ReasonerId::Eq, InferenceCause::DomSingleton);
                if let Some(l) = self.graph.domains.signed_value(SignedVar::plus(v), domains.ub(v)) {
                    domains.set(l, cause)?;
                }
            }
        }
        Ok(())
    }

    pub fn assert_fully_propagated(&mut self, domains: &mut Domains) -> Result<bool, InvalidUpdate> {
        let mut cursor = ObsTrailCursor::new();
        cursor.move_to_end(domains.trail());
        let vars = domains
            .variables()
            .flat_map(|v| [SignedVar::plus(v), SignedVar::minus(v)])
            .collect_vec();
        //
        for v in vars {
            let ub = domains.get_bound(v);
            let new_lit = Lit::from_parts(v, ub);
            self.update_graph(new_lit, domains);
            self.propagate_edge_event(new_lit, domains).unwrap();
            self.propagate_domain_event(v, ub.as_int(), INT_CST_MAX, domains)
                .unwrap();
        }
        while let Some(event) = cursor.pop(domains.trail()) {
            panic!("A propagation event occurred {event:?}");
        }
        Ok(true)
    }

    pub fn add_edge(&mut self, a: impl Into<Node>, b: impl Into<Node>, model: &mut impl ReifyEq) -> Lit {
        let a = a.into();
        let b = b.into();
        if a == b {
            return Lit::TRUE;
        }
        self.graph.add_node(a, model);
        self.graph.add_node(b, model);
        self.graph.label(a, b)
    }
}

impl Backtrack for EqTheory {
    fn save_state(&mut self) -> DecLvl {
        if self.trail.current_decision_level() == DecLvl::ROOT {
            println!("=> en init prop");
            self.print_stats()
        }
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.trail.restore_last_with(|e| match e {
            Event::EdgePropagation { .. } => {}
            Event::EdgeEnabledPos(e) => {
                self.graph.succs_pos.get_mut(&e.src).unwrap().pop().unwrap();
                self.graph.preds_pos.get_mut(&e.tgt).unwrap().pop().unwrap();
            }
            Event::EdgeEnabledNeg(e) => {
                self.graph.succs_neg.get_mut(&e.src).unwrap().pop().unwrap();
                self.graph.preds_neg.get_mut(&e.tgt).unwrap().pop().unwrap();
            }
        })
    }
}

impl Theory for EqTheory {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Eq
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        // if self.cursor.is_pristine() {
        // self.propagate_from_scratch(domains)?;
        // }

        let mut cursor_copy = self.cursor.clone();
        loop {
            let mut new_event_treated = false;

            while let Some(ev) = self.cursor.pop(domains.trail()).copied() {
                self.update_graph(ev.new_literal(), domains);
                if let Some(inference) = ev.cause.as_external_inference() {
                    if inference.writer == self.identity() {
                        let cause = InferenceCause::from(inference.payload);
                        if let InferenceCause::EdgePropagation(_) = cause {
                            continue; // already handled during propagation
                        }
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

        // self.print_stats();
        // TODO: remove (to expensive for casual use in debug mode)
        debug_assert!(self.assert_fully_propagated(domains).unwrap());

        Ok(())
    }

    fn explain(&mut self, l: Lit, context: u32, domains: &Domains, out_explanation: &mut Explanation) {
        let signed_var = l.svar();
        let variable = signed_var.variable();
        let value = l.bound_value().as_int();
        let cause = InferenceCause::from(context);

        match cause {
            InferenceCause::EdgePropagation(event_index) => {
                let event = self.trail.get_event(event_index);
                let &Event::EdgePropagation { x, y, z } = event else {
                    unreachable!()
                };

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
            InferenceCause::DomEq => {
                // inferred x <= v, from a fact  (x = v')   for  v' in ]-oo, v]
                let val_eqs = self.graph.domains.values(signed_var, INT_CST_MIN, value).iter().rev();
                for &l in val_eqs {
                    if domains.entails(l) {
                        out_explanation.push(l);
                        break;
                    }
                }
            }
            InferenceCause::DomNeq if signed_var.is_plus() => {
                let previous = value + 1;
                let neq = !self.graph.domains.value(variable, previous).unwrap_or(Lit::FALSE);
                debug_assert!(domains.entails(neq));
                out_explanation.push(neq);
                let previous_bound = Lit::leq(variable, previous);
                debug_assert!(domains.entails(previous_bound));
                out_explanation.push(previous_bound);
            }
            InferenceCause::DomNeq => {
                debug_assert!(signed_var.is_minus());
                let value = -value;
                let previous = value - 1;
                let neq = !self.graph.domains.value(variable, previous).unwrap_or(Lit::FALSE);
                out_explanation.push(neq); // variable != previous
                debug_assert!(domains.entails(neq));
                let previous_bound = Lit::geq(variable, previous); // variable >= previous
                debug_assert!(domains.entails(previous_bound));
                out_explanation.push(previous_bound);
            }
            InferenceCause::DomUpper => {
                let (var, value) = self.graph.domains.neq_watches(l).next().unwrap();
                out_explanation.push(Lit::leq(var, value - 1))
            }
            InferenceCause::DomLower => {
                let (var, value) = self.graph.domains.neq_watches(l).next().unwrap();
                out_explanation.push(Lit::geq(var, value + 1))
            }
            InferenceCause::DomSingleton => {
                let (var, value) = self.graph.domains.eq_watches(l).next().unwrap();
                out_explanation.push(Lit::geq(var, value));
                out_explanation.push(Lit::leq(var, value));
            }
        }
    }

    fn print_stats(&self) {
        println!("num nodes: {}", self.graph.nodes_ordered.len());
        println!("num edge props1 {}", self.stats.num_edge_propagations);
        println!("num edge props+ {}", self.stats.num_edge_propagations_pos);
        println!("num edge props- {}", self.stats.num_edge_propagations_neg);
        println!("num edge props1 ++  {}", self.stats.num_edge_propagation1_pos_pos);
        println!("num edge props1 +-  {}", self.stats.num_edge_propagation1_pos_neg);
        println!("num edge props1 -+  {}", self.stats.num_edge_propagation1_neg_pos);
        println!("num edge props1 eff  {}", self.stats.num_edge_propagation1_effective);
        println!("num edge props2 ++  {}", self.stats.num_edge_propagation2_pos_pos);
        println!("num edge props2 +-  {}", self.stats.num_edge_propagation2_pos_neg);
        println!("num edge props2 -+  {}", self.stats.num_edge_propagation2_neg_pos);
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
    use crate::model::{Label, Model};
    use crate::reasoners::eq::{EqTheory, InferenceCause, Node, Pair, ReifyEq};
    use crate::reasoners::{Contradiction, Theory};
    use crate::solver::search::random::RandomChoice;
    use crate::solver::Solver;
    use crate::utils::input::Sym;
    use itertools::Itertools;
    use rand::prelude::SmallRng;
    use rand::{Rng, SeedableRng};
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
            theory.propagate(domains).expect("Propagation error");
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
        println!("SETTING !=");
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

        theory.propagate(domains).unwrap();

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
        let symbols = SymbolTable::from(vec![("obj", vec!["alice", "bob", "chloe"])]);
        let symbols = Arc::new(symbols);

        let obj = symbols.types.id_of("obj").unwrap();

        let mut model: Model<S> = Model::new_with_symbols(symbols.clone());
        let vars = ["V", "W", "X", "Y", "Z"]
            .map(|var_name| model.new_sym_var(obj, var_name))
            .iter()
            .copied()
            .collect_vec();

        for (xi, x) in vars.iter().copied().enumerate() {
            for &y in &vars[xi..] {
                model.reify(eq(x, y));
            }
        }

        random_solves(&model, 10, Some(true));
    }

    fn random_solves<S: Label>(model: &Model<S>, num_solves: u64, mut expected_result: Option<bool>) {
        for seed in 0..num_solves {
            let model = model.clone();
            let solver = &mut Solver::new(model);
            solver.set_brancher(RandomChoice::new(seed));
            let solution = solver.solve().unwrap().is_some();
            if let Some(expected_sat) = expected_result {
                assert_eq!(solution, expected_sat)
            }
            // ensure that the next run has the same output
            expected_result = Some(solution)
        }
    }

    fn random_model(seed: u64) -> Model<String> {
        let mut rng = SmallRng::seed_from_u64(seed);
        let objects = vec!["alice", "bob", "chloe", "donald", "elon"];
        let num_objects = rng.gen_range(1..5);
        let objects = objects[0..num_objects].to_vec();
        let symbols = SymbolTable::from(vec![("obj", objects.clone())]);
        let symbols = Arc::new(symbols);

        let obj = symbols.types.id_of("obj").unwrap();

        let mut model: Model<String> = Model::new_with_symbols(symbols.clone());

        let num_scopes = rng.gen_range(0..3);
        let scopes = (0..=num_scopes)
            .into_iter()
            .map(|i| {
                if i == 0 {
                    Lit::TRUE
                } else {
                    model.new_presence_variable(Lit::TRUE, format!("scope_{i}")).true_lit()
                }
            })
            .collect_vec();

        let num_vars = rng.gen_range(0..10);
        println!("Problem num_scopes: {num_scopes}, num_vars:  {num_vars}  num_values: {num_objects}");

        let mut vars = Vec::with_capacity(num_vars);
        for i in 0..num_vars {
            let scope_id = rng.gen_range(0..scopes.len());
            let scope = scopes[scope_id];
            let var_name = format!("x{i}");
            println!("  {var_name} [{scope_id}]  in {:?}", &objects);
            let var = model.new_optional_sym_var(obj, scope, var_name);
            vars.push(var)
        }

        for (xi, x) in vars.iter().copied().enumerate() {
            for &y in &vars[xi..] {
                model.reify(eq(x, y));
            }
        }

        model
    }

    #[test]
    fn random_problems() {
        for seed in 0..100 {
            let model = random_model(seed);
            random_solves(&model, 30, Some(true));
        }
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
