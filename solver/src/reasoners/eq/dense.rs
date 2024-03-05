use crate::backtrack::{Backtrack, DecLvl, EventIndex, ObsTrail, ObsTrailCursor};
use crate::core::literals::Watches;
use crate::core::state::{Domains, Explanation, InvalidUpdate};
use crate::core::{IntCst, Lit, SignedVar, UpperBound, VarRef, INT_CST_MIN};
use crate::model::{Label, Model};
use crate::reasoners::eq::domain;
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use crate::reif::ReifExpr;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};

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

impl DirEdge {
    pub fn id(&self) -> DirEdgeId {
        DirEdgeId {
            src: self.src,
            tgt: self.tgt,
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
    /// Set of directed edges that are already enabled.
    /// Invariant: if an edge is enabled, it must appear exactly once in the
    enabled: HashSet<DirEdgeId>,
    /// Adjacency list for positive & enabled edges (active & label)
    succs_pos: HashMap<Node, Vec<OutEdge>>,
    /// Reverse djacency list for positive & enabled edges (active & label)
    preds_pos: HashMap<Node, Vec<InEdge>>,
    /// Adjacency list for negative & enabled edges (active & !label)
    succs_neg: HashMap<Node, Vec<OutEdge>>,
    /// Reverse adjacency list for negative & enabled edges (active & !label)
    preds_neg: HashMap<Node, Vec<InEdge>>,
    labels: HashMap<DirEdgeId, DirEdgeLabel>,
    watches: Watches<DirEdge>,
}

impl Graph {
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

type DomainEvent = crate::core::state::Event;

#[derive(Clone, Default)]
pub(crate) struct Stats {
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

impl Debug for Stats {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "num edge props1 {}", self.num_edge_propagations)?;
        writeln!(f, "num edge props+ {}", self.num_edge_propagations_pos)?;
        writeln!(f, "num edge props- {}", self.num_edge_propagations_neg)?;
        writeln!(f, "num edge props1 ++  {}", self.num_edge_propagation1_pos_pos)?;
        writeln!(f, "num edge props1 +-  {}", self.num_edge_propagation1_pos_neg)?;
        writeln!(f, "num edge props1 -+  {}", self.num_edge_propagation1_neg_pos)?;
        writeln!(f, "num edge props1 eff  {}", self.num_edge_propagation1_effective)?;
        writeln!(f, "num edge props2 ++  {}", self.num_edge_propagation2_pos_pos)?;
        writeln!(f, "num edge props2 +-  {}", self.num_edge_propagation2_pos_neg)?;
        writeln!(f, "num edge props2 -+  {}", self.num_edge_propagation2_neg_pos)
    }
}

impl std::ops::AddAssign for Stats {
    fn add_assign(&mut self, rhs: Self) {
        self.num_edge_propagations += rhs.num_edge_propagations;
        self.num_edge_propagations_pos += rhs.num_edge_propagations_pos;
        self.num_edge_propagations_neg += rhs.num_edge_propagations_neg;
        self.num_edge_propagation1_pos_pos += rhs.num_edge_propagation1_pos_pos;
        self.num_edge_propagation1_pos_neg += rhs.num_edge_propagation1_pos_neg;
        self.num_edge_propagation1_neg_pos += rhs.num_edge_propagation1_neg_pos;
        self.num_edge_propagation1_effective += rhs.num_edge_propagation1_effective;
        self.num_edge_propagation2_pos_pos += rhs.num_edge_propagation2_pos_pos;
        self.num_edge_propagation2_pos_neg += rhs.num_edge_propagation2_pos_neg;
        self.num_edge_propagation2_neg_pos += rhs.num_edge_propagation2_neg_pos;
    }
}

/// An Equality theory where we force the presence of an edge between each pair of variables.
#[derive(Clone)]
pub struct DenseEqTheory {
    /// Id of the theory, used to disambiguate the source of an inference when multiple dense theories are present
    id: u16,
    graph: Graph,
    /// Cursor of the domains trail
    cursor: ObsTrailCursor<DomainEvent>,
    /// Trail of the events in this EqTheory
    trail: ObsTrail<Event>,
    /// Cursor of the local events
    eq_cursor: ObsTrailCursor<Event>,
    pub(crate) stats: Stats,
}

impl DenseEqTheory {
    pub fn new(id: u16) -> DenseEqTheory {
        DenseEqTheory {
            id,
            graph: Default::default(),
            cursor: Default::default(),
            trail: Default::default(),
            eq_cursor: Default::default(),
            stats: Default::default(),
        }
    }

    pub fn variables(&self) -> impl Iterator<Item = VarRef> + '_ {
        self.graph.nodes_ordered.iter().filter_map(|n| match n {
            Node::Var(v) => Some(*v),
            Node::Val(_) => None,
        })
    }

    /// If this event enables an edge, then add it to the active graph
    fn update_graph(&mut self, event: Lit, domains: &Domains) {
        let watches = self.graph.watches.watches_on(event).collect_vec();
        for e in watches {
            self.try_enable_edge(e, domains);
        }
    }

    /// Adds an edge to the graph.
    /// If the edge is enabled, it will be added to the adjacency lists and marked for propagation.
    fn add_dir_edge(&mut self, src: Node, tgt: Node, label: Lit, active: Lit, model: &mut impl ReifyEq) {
        let de = DirEdge {
            src,
            tgt,
            label,
            active,
        };

        self.graph.watches.add_watch(de.clone(), label);
        self.graph.watches.add_watch(de.clone(), !label);
        self.graph.watches.add_watch(de, active);
        self.graph
            .labels
            .insert(DirEdgeId { src, tgt }, DirEdgeLabel { label, active });
        if let (Node::Var(var), Node::Val(val)) = (src, tgt) {
            let (lb, ub) = model.domain(Node::Var(var));
            if (lb..=ub).contains(&val) {
                self.graph.domains.add_value(var, val, label);
            }
        }

        // attempts to enable the edge
        self.try_enable_edge(de, model.domains());
    }

    pub fn add_node(&mut self, v: impl Into<Node>, model: &mut impl ReifyEq) {
        let v = v.into();
        if self.graph.nodes.contains(&v) {
            return;
        }
        if let Node::Var(_) = v {
            let (lb, ub) = model.domain(v);
            for val in lb..=ub {
                self.add_node(val, model);
            }
        }

        self.graph.succs_pos.insert(v, Vec::new());
        self.graph.preds_pos.insert(v, Vec::new());
        self.graph.succs_neg.insert(v, Vec::new());
        self.graph.preds_neg.insert(v, Vec::new());

        // add edges to all other nodes
        let nodes = self.graph.nodes_ordered.iter().copied().sorted().collect_vec(); // TODO: optimize
        for &other in &nodes {
            let label = model.reify_eq(v, other);
            // the out-edge is active if the presence of tgt implies the presence of v
            let out_active = model.n_presence_implication(other, v);
            self.add_dir_edge(v, other, label, out_active, model);

            let in_active = model.n_presence_implication(v, other);
            self.add_dir_edge(other, v, label, in_active, model);
        }
        self.graph.nodes.insert(v);
        self.graph.nodes_ordered.push(v);
        // println!("  nodes {} edges {}", self.graph.nodes.len(), self.graph.labels.len())
    }

    /// Attempts to enable an edge, returning true if the edge was actually added to the enabled set
    fn try_enable_edge(&mut self, e: DirEdge, domains: &Domains) -> bool {
        if !domains.entails(e.active) || domains.value(e.label).is_none() {
            return false; // edge not enabled
        }
        // println!("  try add: {e:?}");
        if self.graph.enabled.contains(&e.id()) {
            return false; // edge is already enabled
        }
        debug_assert!(self.graph.succs_pos[&e.src]
            .iter()
            .find(|ee| ee.succ == e.tgt)
            .is_none());
        debug_assert!(self.graph.succs_neg[&e.src]
            .iter()
            .find(|ee| ee.succ == e.tgt)
            .is_none());

        match domains.value(e.label) {
            // enable the positive edge
            Some(true) => {
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
                // enable the negative edge
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
            None => {
                unreachable!()
            }
        }
        self.graph.enabled.insert(e.id());
        true // edge added
    }

    /// This edge was just enabled, propagate it
    fn propagate_new_edge(&mut self, e: DirEdge, domains: &mut Domains) -> Result<(), InvalidUpdate> {
        let mut in_to_check: Vec<Node> = Vec::with_capacity(64);
        let mut out_to_check: Vec<Node> = Vec::with_capacity(64);
        self.stats.num_edge_propagations += 1;
        let src = e.src;
        let tgt = e.tgt;

        debug_assert!(domains.entails(e.active));
        debug_assert!(domains.value(e.label).is_some());

        if domains.entails(e.label) {
            self.stats.num_edge_propagations_pos += 1;
            // edge: SRC ===> TGT
            let p_outs = self.graph.succs_pos.remove(&tgt).unwrap();
            for out in &p_outs {
                if out.succ == src {
                    continue;
                }
                // edge: TGT ===> SUCC, enforce SRC ===> SUCC
                self.stats.num_edge_propagation1_pos_pos += 1;
                debug_assert!(domains.entails(out.active));
                debug_assert!(domains.entails(out.label));
                match self.set_edge_label(true, src, tgt, out.succ, domains) {
                    Err(e) => {
                        self.graph.succs_pos.insert(tgt, p_outs).map(|_| unreachable!()); // restore
                        return Err(e);
                    }
                    Ok(true) => {
                        out_to_check.push(out.succ);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                    Ok(false) => {}
                }
            }
            self.graph.succs_pos.insert(tgt, p_outs).map(|_| unreachable!()); // restore

            let n_outs = self.graph.succs_neg.remove(&tgt).unwrap();
            for out in &n_outs {
                if out.succ == src {
                    continue;
                }
                // edge TGT =!=> SUCC, enforce SRC =!=> SUCC
                self.stats.num_edge_propagation1_pos_neg += 1;
                debug_assert!(domains.entails(out.active));
                debug_assert!(domains.entails(!out.label));
                match self.set_edge_label(false, src, tgt, out.succ, domains) {
                    Err(e) => {
                        self.graph.succs_neg.insert(tgt, n_outs).map(|_| unreachable!()); //restore
                        return Err(e);
                    }
                    Ok(true) => {
                        out_to_check.push(out.succ);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                    Ok(false) => {}
                }
            }
            self.graph.succs_neg.insert(tgt, n_outs).map(|_| unreachable!());

            let p_ins = self.graph.preds_pos.remove(&src).unwrap();
            for inc in &p_ins {
                if inc.pred == tgt {
                    continue;
                }
                self.stats.num_edge_propagation1_pos_pos += 1;
                debug_assert!(domains.entails(inc.active));
                debug_assert!(domains.entails(inc.label));
                // edge: PRED ==> SRC, enforce PRED ===> TGT
                match self.set_edge_label(true, inc.pred, src, tgt, domains) {
                    Err(e) => {
                        self.graph.preds_pos.insert(src, p_ins).map(|_| unreachable!()); // restore
                        return Err(e);
                    }
                    Ok(true) => {
                        in_to_check.push(inc.pred);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                    Ok(false) => {}
                }
            }
            self.graph.preds_pos.insert(src, p_ins).map(|_| unreachable!());

            let n_ins = self.graph.preds_neg.remove(&src).unwrap();
            for inc in &n_ins {
                if inc.pred == tgt {
                    continue;
                }
                self.stats.num_edge_propagation1_pos_neg += 1;
                debug_assert!(domains.entails(inc.active));
                debug_assert!(!domains.entails(inc.label));
                // edge: PRED =!> SRC, enforce PRED =!=> TGT
                match self.set_edge_label(false, inc.pred, src, tgt, domains) {
                    Err(e) => {
                        self.graph.preds_neg.insert(src, n_ins).map(|_| unreachable!()); // restore
                        return Err(e);
                    }
                    Ok(true) => {
                        in_to_check.push(inc.pred);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                    Ok(false) => {}
                }
            }
            self.graph.preds_neg.insert(src, n_ins).map(|_| unreachable!());
        } else {
            debug_assert!(domains.entails(!e.label));
            self.stats.num_edge_propagations_neg += 1;
            // edge: SRC =!=> TGT
            let p_outs = self.graph.succs_pos.remove(&tgt).unwrap();
            for out in &p_outs {
                self.stats.num_edge_propagation1_neg_pos += 1;
                debug_assert!(domains.entails(out.active));
                debug_assert!(domains.entails(out.label));
                // edge: TGT ===> SUCC, enforce SRC =!=> SUCC
                match self.set_edge_label(false, src, tgt, out.succ, domains) {
                    Err(e) => {
                        self.graph.succs_pos.insert(tgt, p_outs).map(|_| unreachable!()); // restore
                        return Err(e);
                    }
                    Ok(true) => {
                        out_to_check.push(out.succ);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                    Ok(false) => {}
                }
            }
            self.graph.succs_pos.insert(tgt, p_outs).map(|_| unreachable!());

            let p_ins = self.graph.preds_pos.remove(&src).unwrap();
            for inc in &p_ins {
                self.stats.num_edge_propagation1_neg_pos += 1;
                debug_assert!(domains.entails(inc.active));
                debug_assert!(domains.entails(inc.label));
                // edge: PRED ==> SRC, enforce PRED =!=> TGT
                match self.set_edge_label(false, inc.pred, src, tgt, domains) {
                    Err(e) => {
                        self.graph.preds_pos.insert(src, p_ins).map(|_| unreachable!()); // restore
                        return Err(e);
                    }
                    Ok(true) => {
                        in_to_check.push(inc.pred);
                        self.stats.num_edge_propagation1_effective += 1;
                    }
                    Ok(false) => {}
                }
            }
            self.graph.preds_pos.insert(src, p_ins).map(|_| unreachable!());
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
                self.set_edge_label(true, x, y, z, domains)?;
            }
            for &z in &yzs_neg {
                if x == z {
                    continue;
                }
                self.stats.num_edge_propagation2_pos_neg += 1;
                self.set_edge_label(false, x, y, z, domains)?;
            }
        }

        for x in xys_neg {
            // x =!=> y
            for &z in &yzs_pos {
                if x == z {
                    continue;
                }
                self.set_edge_label(false, x, y, z, domains)?;
                self.stats.num_edge_propagation2_neg_pos += 1;
            }
        }
        Ok(())
    }

    /// Sets the label of XZ, recording XYZ as the explanation for this change.
    fn set_edge_label(
        &mut self,
        value: bool,
        x: Node,
        y: Node,
        z: Node,
        domains: &mut Domains,
    ) -> Result<bool, InvalidUpdate> {
        let label = self.graph.label(x, z);
        let label = if value { label } else { !label };

        debug_assert!(
            domains.entails(self.graph.active(x, z)),
            "xz not active when xy and yz are"
        );

        match domains.value(label) {
            Some(true) => Ok(false),
            _ => {
                // there might be a change, record event source to be able to explain it
                let event = Event::EdgePropagation { x, y, z };
                let id = self.trail.push(event);
                let cause = self.identity().cause(InferenceCause::EdgePropagation(id));
                if domains.set(label, cause)? {
                    let id = DirEdgeId { src: x, tgt: z };
                    let lbl = self.graph.labels[&id];
                    let edge = DirEdge {
                        src: id.src,
                        tgt: id.tgt,
                        label: lbl.label,
                        active: lbl.active,
                    };
                    Ok(self.try_enable_edge(edge, domains))
                } else {
                    Ok(false)
                }
            }
        }
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
            let cause = self.identity().cause(InferenceCause::DomEq);
            domains.set_lb(var, value, cause)?;
            domains.set_ub(var, value, cause)?;
        }
        for (var, value) in self.graph.domains.neq_watches(new_literal) {
            let cause = self.identity().cause(InferenceCause::DomNeq);
            if domains.lb(var) == value {
                domains.set_lb(var, value + 1, cause)?;
            } else if domains.ub(var) == value {
                domains.set_ub(var, value - 1, cause)?;
            }
        }

        if self.graph.domains.has_domain(v.variable()) {
            for &invalid in self.graph.domains.values(v, new_ub + 1, previous_ub) {
                let cause = if v.is_plus() {
                    self.identity().cause(InferenceCause::DomUpper)
                } else {
                    // dbg!(invalid, v, new_ub + 1, previous_ub);
                    self.identity().cause(InferenceCause::DomLower)
                };
                domains.set(!invalid, cause)?;
            }

            // reduce domain if the upper bound is excluded
            let mut updated_ub = new_ub;
            while let Some(l) = self.graph.domains.signed_value(v, updated_ub) {
                if domains.entails(!l) {
                    updated_ub -= 1;
                    let cause = self.identity().cause(InferenceCause::DomNeq);
                    domains.set(Lit::from_parts(v, UpperBound::ub(updated_ub)), cause)?;
                } else {
                    break;
                }
            }

            let v = v.variable();
            if domains.lb(v) == domains.ub(v) {
                let cause = self.identity().cause(InferenceCause::DomSingleton);
                if let Some(l) = self.graph.domains.signed_value(SignedVar::plus(v), domains.ub(v)) {
                    domains.set(l, cause)?;
                }
            }
        }
        Ok(())
    }

    pub fn add_edge(&mut self, a: impl Into<Node>, b: impl Into<Node>, model: &mut impl ReifyEq) -> Lit {
        let a = a.into();
        let b = b.into();
        if a == b {
            return Lit::TRUE;
        }
        self.add_node(a, model);
        self.add_node(b, model);
        self.graph.label(a, b)
    }
}

impl Backtrack for DenseEqTheory {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        use crate::backtrack::BacktrackWith;
        self.trail.restore_last_with(|e| match e {
            Event::EdgePropagation { .. } => {}
            Event::EdgeEnabledPos(e) => {
                self.graph.succs_pos.get_mut(&e.src).unwrap().pop().unwrap();
                self.graph.preds_pos.get_mut(&e.tgt).unwrap().pop().unwrap();
                assert!(self.graph.enabled.remove(&e.id()));
            }
            Event::EdgeEnabledNeg(e) => {
                self.graph.succs_neg.get_mut(&e.src).unwrap().pop().unwrap();
                self.graph.preds_neg.get_mut(&e.tgt).unwrap().pop().unwrap();
                assert!(self.graph.enabled.remove(&e.id()));
            }
        })
    }
}

impl Theory for DenseEqTheory {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Eq(self.id)
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        loop {
            let mut new_event_treated = false;
            while let Some(ev) = self.eq_cursor.pop(&self.trail) {
                match ev {
                    Event::EdgePropagation { .. } => {}
                    Event::EdgeEnabledPos(e) => {
                        self.propagate_new_edge(*e, domains)?;
                    }
                    Event::EdgeEnabledNeg(e) => {
                        self.propagate_new_edge(*e, domains)?;
                    }
                }
                new_event_treated = true;
            }

            // just handle a single one
            if let Some(ev) = self.cursor.pop(domains.trail()).copied() {
                self.update_graph(ev.new_literal(), domains);
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

    fn explain(
        &mut self,
        l: Lit,
        context: crate::core::state::InferenceCause,
        domains: &Domains,
        out_explanation: &mut Explanation,
    ) {
        debug_assert_eq!(context.writer, self.identity());

        let signed_var = l.svar();
        let variable = signed_var.variable();
        let value = l.bound_value().as_int();
        let cause = InferenceCause::from(context.payload);

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
    fn domains(&self) -> &Domains;
    fn domain(&self, a: Node) -> (IntCst, IntCst);
    fn reify_eq(&mut self, a: Node, b: Node) -> Lit;

    /// Return a literal that is true iff p(a) => p(b)
    fn presence_implication(&self, a: VarRef, b: VarRef) -> Lit;

    fn n_presence_implication(&self, a: Node, b: Node) -> Lit {
        self.presence_implication(var_of(a), var_of(b))
    }
}

impl<L: Label> ReifyEq for Model<L> {
    fn domains(&self) -> &Domains {
        &self.state
    }
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
    use crate::reasoners::eq::dense::InferenceCause;
    use crate::reasoners::eq::{DenseEqTheory, Node, ReifyEq};
    use crate::reasoners::{Contradiction, Theory};
    use crate::solver::search::random::RandomChoice;
    use crate::solver::Solver;
    use crate::utils::input::Sym;
    use itertools::Itertools;
    use rand::prelude::SmallRng;
    use rand::{Rng, SeedableRng};
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

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

    struct Eqs {
        domains: Domains,
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
            Eqs {
                domains: domains.clone(),
                map,
            }
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

        fn domains(&self) -> &Domains {
            &self.domains
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
        let mut theory = DenseEqTheory::new(0);
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
        theory.print_stats();
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
        let mut theory = DenseEqTheory::new(0);
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
            expected_result = Some(solution);
            solver.print_stats();
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
