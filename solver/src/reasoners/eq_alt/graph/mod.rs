use std::array;
use std::fmt::{Debug, Display};
use std::hash::Hash;

use folds::{EmptyTag, EqFold, EqOrNeqFold, ReducingFold};
use hashbrown::HashSet;
use itertools::Itertools;
use node_store::NodeStore;
pub use traversal::TaggedNode;

use crate::backtrack::{Backtrack, DecLvl, Trail};
use crate::collections::set::IterableRefSet;
use crate::core::Lit;
use crate::create_ref_type;
use crate::reasoners::eq_alt::graph::{adj_list::EqAdjList, traversal::GraphTraversal};

use super::node::Node;
use super::propagators::Propagator;
use super::relation::EqRelation;

mod adj_list;
pub mod folds;
mod node_store;
pub mod subsets;
pub mod traversal;

create_ref_type!(NodeId);
pub use node_store::GroupId;

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
pub struct IdEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub active: Lit,
    pub relation: EqRelation,
}

impl IdEdge {
    fn new(source: NodeId, target: NodeId, active: Lit, relation: EqRelation) -> Self {
        Self {
            source,
            target,
            active,
            relation,
        }
    }

    /// Should only be used for reverse adjacency graph. Propagator id is not reversed.
    fn reverse(&self) -> Self {
        IdEdge {
            source: self.target,
            target: self.source,
            ..*self
        }
    }
}

#[derive(Clone)]
enum Event {
    EdgeAdded(IdEdge),
    GroupEdgeAdded(IdEdge),
    GroupEdgeRemoved(IdEdge),
}

#[derive(Clone, Default)]
pub(super) struct DirEqGraph {
    pub node_store: NodeStore,
    outgoing: EqAdjList,
    incoming: EqAdjList,
    outgoing_grouped: EqAdjList,
    incoming_grouped: EqAdjList,
    trail: Trail<Event>,
}

impl DirEqGraph {
    pub fn new() -> Self {
        Default::default()
    }

    /// Add node to graph if not present. Returns the id of the Node.
    pub fn insert_node(&mut self, node: Node) -> NodeId {
        self.node_store
            .get_id(&node)
            .unwrap_or_else(|| self.node_store.insert_node(node))
    }

    /// Get node from id.
    ///
    /// # Panics
    ///
    /// Panics if node with `id` is not in graph.
    pub fn get_node(&self, id: NodeId) -> Node {
        self.node_store.get_node(id)
    }

    pub fn get_id(&self, node: &Node) -> Option<NodeId> {
        self.node_store.get_id(node)
    }

    pub fn get_group_id(&self, id: NodeId) -> GroupId {
        self.node_store.get_group_id(id)
    }

    #[allow(unused)]
    pub fn get_group(&self, id: GroupId) -> Vec<NodeId> {
        self.node_store.get_group(id)
    }

    pub fn get_group_nodes(&self, id: GroupId) -> Vec<Node> {
        self.node_store.get_group_nodes(id)
    }

    pub fn merge(&mut self, ids: (NodeId, NodeId)) {
        let child = self.get_group_id(ids.0);
        let parent = self.get_group_id(ids.1);
        self.node_store.merge(child, parent);

        for edge in self.outgoing_grouped.iter_edges(child.into()).cloned().collect_vec() {
            self.trail.push(Event::GroupEdgeRemoved(edge));
            self.outgoing_grouped.remove_edge(edge);
            self.incoming_grouped.remove_edge(edge.reverse());

            let new_edge = IdEdge {
                source: parent.into(),
                ..edge
            };
            // Avoid adding edges from a group into the same group
            if new_edge.source == new_edge.target {
                continue;
            }

            let added = self.outgoing_grouped.insert_edge(new_edge);
            assert_eq!(added, self.incoming_grouped.insert_edge(new_edge.reverse()));
            if added {
                self.trail.push(Event::GroupEdgeAdded(new_edge));
            }
        }

        for edge in self.incoming_grouped.iter_edges(child.into()).cloned().collect_vec() {
            let edge = edge.reverse();
            self.trail.push(Event::GroupEdgeRemoved(edge));
            self.outgoing_grouped.remove_edge(edge);
            self.incoming_grouped.remove_edge(edge.reverse());

            let new_edge = IdEdge {
                target: parent.into(),
                ..edge
            };
            // Avoid adding edges from a group into the same group
            if new_edge.source == new_edge.target {
                continue;
            }

            let added = self.outgoing_grouped.insert_edge(new_edge);
            assert_eq!(added, self.incoming_grouped.insert_edge(new_edge.reverse()));
            if added {
                self.trail.push(Event::GroupEdgeAdded(new_edge));
            }
        }
    }

    pub fn group_product(&self, source_id: GroupId, target_id: GroupId) -> impl Iterator<Item = (Node, Node)> {
        let sources = self.get_group_nodes(source_id);
        let targets = self.get_group_nodes(target_id);
        sources.into_iter().cartesian_product(targets)
    }

    /// Returns an edge from a propagator without adding it to the graph.
    ///
    /// Adds the nodes to the graph if they are not present.
    pub fn create_edge(&mut self, prop: &Propagator) -> IdEdge {
        let source_id = self.insert_node(prop.a);
        let target_id = self.insert_node(prop.b);
        IdEdge::new(source_id, target_id, prop.enabler.active, prop.relation)
    }

    /// Adds an edge to the graph.
    pub fn add_edge(&mut self, edge: IdEdge) {
        self.trail.push(Event::EdgeAdded(edge));
        self.outgoing.insert_edge(edge);
        self.incoming.insert_edge(edge.reverse());
        let grouped_edge = IdEdge {
            source: self.get_group_id(edge.source).into(),
            target: self.get_group_id(edge.target).into(),
            ..edge
        };
        self.trail.push(Event::GroupEdgeAdded(grouped_edge));
        self.outgoing_grouped.insert_edge(grouped_edge);
        self.incoming_grouped.insert_edge(grouped_edge.reverse());
    }

    fn get_dir(&self, dir: GraphDir) -> &EqAdjList {
        match dir {
            GraphDir::Forward => &self.outgoing,
            GraphDir::Reverse => &self.incoming,
            GraphDir::ForwardGrouped => &self.outgoing_grouped,
            GraphDir::ReverseGrouped => &self.incoming_grouped,
        }
    }

    pub fn get_out_edges(&self, node: NodeId, dir: GraphDir) -> Vec<IdEdge> {
        self.get_dir(dir)
            .get_edges(node)
            .map(|s| s.into_iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_traversal_graph(&self, dir: GraphDir) -> impl traversal::Graph + use<'_> {
        self.get_dir(dir)
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = Node> + use<'_> {
        self.outgoing.iter_nodes().map(|id| self.node_store.get_node(id))
    }

    /// Get all paths which would require the given edge to exist.
    ///
    /// For an edge x -==-> y, returns a vec of all pairs (w, z) such that w -=-> z or w -!=-> z in G union x -=-> y, but not in G.
    ///
    /// For an edge x -!=-> y, returns a vec of all pairs (w, z) such that w -!=> z in G union x -!=-> y, but not in G.
    /// propagator nodes must already be added
    pub fn paths_requiring(&self, edge: IdEdge) -> Vec<Path> {
        // Convert edge to edge between groups
        let edge = IdEdge {
            source: self.node_store.get_group_id(edge.source).into(),
            target: self.node_store.get_group_id(edge.target).into(),
            ..edge
        };

        match edge.relation {
            EqRelation::Eq => self.paths_requiring_eq(edge),
            EqRelation::Neq => self.paths_requiring_neq(edge),
        }
    }

    /// NOTE: This set will only contain representatives, not any node.
    ///
    /// TODO: Return a reference to the set if possible (maybe box)
    fn reachable_set(&self, adj_list: &EqAdjList, source: NodeId) -> IterableRefSet<TaggedNode<EqRelation>> {
        let mut traversal = GraphTraversal::new(adj_list, EqOrNeqFold(), source, false);
        // Consume iterator
        for _ in traversal.by_ref() {}
        traversal.get_reachable().clone()
    }

    fn reachable_set_excluding(
        &self,
        adj_list: &EqAdjList,
        source: NodeId,
        exclude: TaggedNode<EqRelation>,
    ) -> Option<IterableRefSet<TaggedNode<EqRelation>>> {
        let mut traversal = GraphTraversal::new(adj_list, EqOrNeqFold(), source, false);
        // Consume iterator
        if traversal.contains(&exclude) {
            None
        } else {
            Some(traversal.get_reachable().clone())
        }
    }

    fn paths_requiring_eq(&self, edge: IdEdge) -> Vec<Path> {
        let Some(reachable_preds) = self.reachable_set_excluding(
            &self.incoming_grouped,
            edge.target,
            TaggedNode(edge.source, EqRelation::Eq),
        ) else {
            return Vec::new();
        };
        let reachable_succs = self.reachable_set(&self.outgoing_grouped, edge.source);
        debug_assert!(!reachable_succs.contains(TaggedNode(edge.target, EqRelation::Eq)));

        let predecessors = GraphTraversal::new(
            &self.incoming_grouped,
            ReducingFold::new(&reachable_preds, EqOrNeqFold()),
            edge.source,
            false,
        );

        let successors = GraphTraversal::new(
            &self.outgoing_grouped,
            ReducingFold::new(&reachable_succs, EqOrNeqFold()),
            edge.target,
            false,
        )
        .collect_vec();

        predecessors
            .into_iter()
            .cartesian_product(successors)
            .filter_map(
                |(TaggedNode(pred_id, pred_relation), TaggedNode(succ_id, succ_relation))| {
                    // pred id and succ id are GroupIds since all above graph traversals are on MergedGraphs
                    Some(Path::new(
                        pred_id.into(),
                        succ_id.into(),
                        (pred_relation + succ_relation)?,
                    ))
                },
            )
            .collect_vec()
    }

    fn paths_requiring_neq_partial<'a>(
        &'a self,
        rev_set: &'a IterableRefSet<TaggedNode<EmptyTag>>,
        fwd_set: &'a IterableRefSet<TaggedNode<EmptyTag>>,
        source: NodeId,
        target: NodeId,
    ) -> impl Iterator<Item = Path> + use<'a> {
        let predecessors = GraphTraversal::new(
            &self.incoming_grouped,
            ReducingFold::new(rev_set, EqFold()),
            source,
            false,
        );

        let successors = GraphTraversal::new(
            &self.outgoing_grouped,
            ReducingFold::new(fwd_set, EqFold()),
            target,
            false,
        )
        .collect_vec();

        predecessors.cartesian_product(successors).map(
            // pred id and succ id are GroupIds since all above graph traversals are on MergedGraphs
            |(TaggedNode(pred_id, ..), TaggedNode(succ_id, ..))| {
                Path::new(pred_id.into(), succ_id.into(), EqRelation::Neq)
            },
        )
    }

    fn paths_requiring_neq(&self, edge: IdEdge) -> Vec<Path> {
        let Some(reachable_preds) = self.reachable_set_excluding(
            &self.incoming_grouped,
            edge.target,
            TaggedNode(edge.source, EqRelation::Neq),
        ) else {
            return Vec::new();
        };
        let reachable_succs = self.reachable_set(&self.outgoing_grouped, edge.source);
        let [mut reachable_preds_eq, mut reachable_preds_neq, mut reachable_succs_eq, mut reachable_succs_neq] =
            array::from_fn(|_| IterableRefSet::new());

        for e in reachable_succs.iter() {
            match e.1 {
                EqRelation::Eq => reachable_succs_eq.insert(TaggedNode(e.0, EmptyTag())),
                EqRelation::Neq => reachable_succs_neq.insert(TaggedNode(e.0, EmptyTag())),
            }
        }
        for e in reachable_preds.iter() {
            match e.1 {
                EqRelation::Eq => reachable_preds_eq.insert(TaggedNode(e.0, EmptyTag())),
                EqRelation::Neq => reachable_preds_neq.insert(TaggedNode(e.0, EmptyTag())),
            }
        }

        let mut res =
            self.paths_requiring_neq_partial(&reachable_preds_eq, &reachable_succs_neq, edge.source, edge.target);

        // Edge will be duplicated otherwise
        res.next().unwrap();

        res.chain(self.paths_requiring_neq_partial(&reachable_preds_neq, &reachable_succs_eq, edge.source, edge.target))
            .collect_vec()
    }

    #[allow(unused)]
    pub fn to_graphviz(&self) -> String {
        let mut strings = vec!["Ungrouped: ".to_string(), "digraph {".to_string()];
        for e in self.outgoing.iter_all_edges() {
            strings.push(format!(
                "  {} -> {} [label=\"{} ({:?})\"]",
                e.source.to_u32(),
                e.target.to_u32(),
                e.relation,
                e.active
            ));
        }
        strings.push("}".to_string());
        strings.join("\n")
    }

    #[allow(unused)]
    pub fn to_graphviz_grouped(&self) -> String {
        let mut strings = vec!["Grouped: ".to_string(), "digraph {".to_string()];
        for e in self.outgoing_grouped.iter_all_edges() {
            strings.push(format!(
                "  {} -> {} [label=\"{} ({:?})\"]",
                e.source.to_u32(),
                e.target.to_u32(),
                e.relation,
                e.active
            ));
        }
        strings.push("}".to_string());
        strings.join("\n")
    }

    #[allow(unused)]
    pub fn print_merge_statistics(&self) {
        println!("Total nodes: {}", self.node_store.len());
        println!("Total groups: {}", self.node_store.count_groups());
        println!("Outgoing edges: {}", self.outgoing.iter_all_edges().count());
        println!(
            "Outgoing_grouped edges: {}",
            self.outgoing_grouped.iter_all_edges().count()
        );
    }

    /// Check that nodes that are not group representatives are not group reps
    #[allow(unused)]
    pub fn verify_grouping(&self) {
        let groups = self.node_store.groups().into_iter().collect::<HashSet<_>>();
        for node in self.node_store.nodes() {
            if groups.contains(&GroupId::from(node)) {
                continue;
            }
            if let Some(out_edges) = self.outgoing_grouped.get_edges(node) {
                if !out_edges.is_empty() {
                    panic!()
                }
            }
            if let Some(out_edges) = self.incoming_grouped.get_edges(node) {
                if !out_edges.is_empty() {
                    panic!()
                }
            }
        }
    }
}

impl Backtrack for DirEqGraph {
    fn save_state(&mut self) -> DecLvl {
        self.node_store.save_state();
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.node_store.restore_last();
        self.trail.restore_last_with(|event| match event {
            Event::EdgeAdded(edge) => {
                self.outgoing.remove_edge(edge);
                self.incoming.remove_edge(edge.reverse());
            }
            Event::GroupEdgeAdded(edge) => {
                self.outgoing_grouped.remove_edge(edge);
                self.incoming_grouped.remove_edge(edge.reverse());
            }
            Event::GroupEdgeRemoved(edge) => {
                self.outgoing_grouped.insert_edge(edge);
                self.incoming_grouped.insert_edge(edge.reverse());
            }
        });
    }
}

/// Directed pair of nodes with a == or != relation
#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Path {
    pub source_id: GroupId,
    pub target_id: GroupId,
    pub relation: EqRelation,
}

impl Debug for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let &Path {
            source_id,
            target_id,
            relation,
        } = self;
        write!(f, "{source_id:?} --{relation}--> {target_id:?}")
    }
}

impl Path {
    pub fn new(source: GroupId, target: GroupId, relation: EqRelation) -> Self {
        Self {
            source_id: source,
            target_id: target,
            relation,
        }
    }
}

pub enum GraphDir {
    Forward,
    Reverse,
    ForwardGrouped,
    #[allow(unused)]
    ReverseGrouped,
}

#[cfg(test)]
mod tests {
    use EqRelation::*;

    use crate::reasoners::eq_alt::graph::folds::EmptyTag;

    use super::{traversal::NodeTag, *};

    macro_rules! assert_eq_unordered_unique {
        ($left:expr, $right:expr $(,)?) => {{
            use std::collections::HashSet;
            let left = $left.into_iter().collect_vec();
            let right = $right.into_iter().collect_vec();
            assert!(
                left.clone().into_iter().all_unique(),
                "{:?} is duplicated in left",
                left.clone().into_iter().duplicates().collect_vec()
            );
            assert!(
                right.clone().into_iter().all_unique(),
                "{:?} is duplicated in right",
                right.clone().into_iter().duplicates().collect_vec()
            );
            let l_set: HashSet<_> = left.into_iter().collect();
            let r_set: HashSet<_> = right.into_iter().collect();

            let lr_diff: HashSet<_> = l_set.difference(&r_set).cloned().collect();
            let rl_diff: HashSet<_> = r_set.difference(&l_set).cloned().collect();

            assert!(lr_diff.is_empty(), "Found in left but not in right: {:?}", lr_diff);
            assert!(rl_diff.is_empty(), "Found in right but not in left: {:?}", rl_diff);
        }};
    }

    fn prop(src: i32, tgt: i32, relation: EqRelation) -> Propagator {
        Propagator::new(Node::Val(src), Node::Val(tgt), relation, Lit::TRUE, Lit::TRUE)
    }

    fn id(g: &DirEqGraph, node: i32) -> NodeId {
        g.get_id(&Node::Val(node)).unwrap()
    }

    fn edge(g: &DirEqGraph, src: i32, tgt: i32, relation: EqRelation) -> IdEdge {
        IdEdge::new(
            g.get_id(&Node::Val(src)).unwrap(),
            g.get_id(&Node::Val(tgt)).unwrap(),
            Lit::TRUE,
            relation,
        )
    }

    fn tn<T: NodeTag>(g: &DirEqGraph, node: i32, tag: T) -> TaggedNode<T> {
        TaggedNode(id(g, node), tag)
    }

    fn path(g: &DirEqGraph, src: i32, tgt: i32, relation: EqRelation) -> Path {
        Path::new(
            g.get_id(&Node::Val(src)).unwrap().into(),
            g.get_id(&Node::Val(tgt)).unwrap().into(),
            relation,
        )
    }

    /* Copy this into https://magjac.com/graphviz-visual-editor/
    digraph {
       0 -> 1 [label=" ="]
       1 -> 2 [label=" !="]
       1 -> 3 [label=" ="]
       2 -> 4 [label=" !="]
       1 -> 5 [label=" ="]
       5 -> 6 [label=" ="]
       6 -> 0 [label=" !="]
       5 -> 0 [label=" ="]
    }
    */
    fn instance1() -> DirEqGraph {
        let mut g = DirEqGraph::new();
        for prop in [
            prop(0, 1, Eq),
            prop(1, 2, Neq),
            prop(1, 3, Eq),
            prop(2, 4, Neq),
            prop(1, 5, Eq),
            prop(5, 6, Eq),
            prop(6, 0, Neq),
            prop(5, 0, Eq),
        ] {
            let edge = g.create_edge(&prop);
            g.add_edge(edge);
        }
        g
    }

    /* Instance focused on merging
    digraph {
        0 -> 1 [label=" ="]
        1 -> 0 [label=" ="]
        1 -> 2 [label=" ="]
        2 -> 0 [label=" ="]
        2 -> 3 [label=" !="]
        3 -> 4 [label=" ="]
        4 -> 5 [label=" ="]
        5 -> 3 [label=" ="]
        0 -> 5 [label=" !="]
        4 -> 1 [label=" ="]
    }
    */
    fn instance2() -> DirEqGraph {
        let mut g = DirEqGraph::new();
        for prop in [
            prop(0, 1, Eq),
            prop(1, 0, Eq),
            prop(1, 2, Eq),
            prop(2, 0, Eq),
            prop(2, 3, Neq),
            prop(3, 4, Eq),
            prop(4, 5, Eq),
            prop(5, 3, Eq),
            prop(0, 5, Neq),
            prop(4, 1, Eq),
        ] {
            let edge = g.create_edge(&prop);
            g.add_edge(edge);
        }
        g
    }

    #[test]
    fn test_traversal() {
        let g = instance1();

        let traversal = GraphTraversal::new(&g.outgoing, EqFold(), id(&g, 0), false);
        assert_eq_unordered_unique!(
            traversal,
            vec![
                tn(&g, 0, EmptyTag()),
                tn(&g, 1, EmptyTag()),
                tn(&g, 3, EmptyTag()),
                tn(&g, 5, EmptyTag()),
                tn(&g, 6, EmptyTag()),
            ],
        );

        let traversal = GraphTraversal::new(&g.outgoing, EqFold(), id(&g, 6), false);
        assert_eq_unordered_unique!(traversal, vec![tn(&g, 6, EmptyTag())]);

        let traversal = GraphTraversal::new(&g.incoming, EqOrNeqFold(), id(&g, 0), false);
        assert_eq_unordered_unique!(
            traversal,
            vec![
                tn(&g, 0, Eq),
                tn(&g, 6, Neq),
                tn(&g, 5, Eq),
                tn(&g, 5, Neq),
                tn(&g, 1, Eq),
                tn(&g, 1, Neq),
                tn(&g, 0, Neq),
            ],
        );
    }

    #[test]
    fn test_merging() {
        let mut g = instance1();
        g.merge((id(&g, 0), id(&g, 1)));
        g.merge((id(&g, 5), id(&g, 1)));
        let rep = g.get_group_id(id(&g, 0));
        let Node::Val(rep) = g.get_node(rep.into()) else {
            panic!()
        };
        assert_eq_unordered_unique!(
            g.outgoing_grouped.get_edges(id(&g, rep)).unwrap().into_iter().cloned(),
            vec![edge(&g, rep, 6, Eq), edge(&g, rep, 3, Eq), edge(&g, rep, 2, Neq)]
        );
        assert_eq_unordered_unique!(
            g.incoming_grouped.get_edges(id(&g, rep)).unwrap().into_iter().cloned(),
            vec![edge(&g, rep, 6, Neq)]
        );
    }

    #[test]
    fn test_reduced_path() {
        let g = instance2();
        let mut traversal = GraphTraversal::new(&g.outgoing, EqOrNeqFold(), id(&g, 0), true);
        let target = traversal
            .find(|&TaggedNode(n, r)| n == id(&g, 4) && r == Neq)
            .expect("Path exists");

        let path1 = vec![edge(&g, 3, 4, Eq), edge(&g, 5, 3, Eq), edge(&g, 0, 5, Neq)];
        let path2 = vec![
            edge(&g, 3, 4, Eq),
            edge(&g, 2, 3, Neq),
            edge(&g, 1, 2, Eq),
            edge(&g, 0, 1, Eq),
        ];
        let mut set = IterableRefSet::new();
        if traversal.get_path(target) == path1 {
            set.insert(TaggedNode(id(&g, 5), Neq));
            let mut traversal =
                GraphTraversal::new(&g.outgoing, ReducingFold::new(&set, EqOrNeqFold()), id(&g, 0), true);
            let target = traversal
                .find(|&TaggedNode(n, r)| n == id(&g, 4) && r == Neq)
                .expect("Path exists");
            assert_eq!(traversal.get_path(target), path2);
        } else if traversal.get_path(target) == path2 {
            set.insert(TaggedNode(id(&g, 1), Eq));
            let mut traversal =
                GraphTraversal::new(&g.outgoing, ReducingFold::new(&set, EqOrNeqFold()), id(&g, 0), true);
            let target = traversal
                .find(|&TaggedNode(n, r)| n == id(&g, 4) && r == Neq)
                .expect("Path exists");
            assert_eq!(traversal.get_path(target), path1);
        }
    }

    #[test]
    fn test_paths_requiring_cycles() {
        let mut g = DirEqGraph::new();
        for i in -3..=3 {
            g.insert_node(Node::Val(i));
        }

        g.add_edge(edge(&g, -3, -2, Eq));
        g.add_edge(edge(&g, -2, -1, Eq));
        assert_eq_unordered_unique!(
            g.paths_requiring(edge(&g, -1, -3, Eq)),
            [
                path(&g, -2, -2, Eq),
                path(&g, -1, -3, Eq),
                path(&g, -1, -2, Eq),
                path(&g, -2, -3, Eq)
            ]
        );
        g.add_edge(edge(&g, -1, -3, Eq));
        g.merge((id(&g, -1), id(&g, -3)));
        g.merge((id(&g, -2), id(&g, -3)));
        assert_eq_unordered_unique!(g.paths_requiring(edge(&g, -1, -3, Eq)), []);
        assert_eq_unordered_unique!(g.paths_requiring(edge(&g, -3, -3, Eq)), []);

        g.add_edge(edge(&g, 0, 1, Eq));
        assert_eq_unordered_unique!(g.paths_requiring(edge(&g, 1, 0, Eq)), [path(&g, 1, 0, Eq)]);

        assert_eq_unordered_unique!(
            g.paths_requiring(edge(&g, 1, 0, Neq)),
            [path(&g, 1, 0, Neq), path(&g, 0, 0, Neq), path(&g, 1, 1, Neq)]
        );

        g.add_edge(edge(&g, 2, 3, Neq));
        assert_eq_unordered_unique!(
            g.paths_requiring(edge(&g, 3, 2, Eq)),
            [path(&g, 3, 2, Eq), path(&g, 2, 2, Neq), path(&g, 3, 3, Neq)]
        );
    }

    #[test]
    fn test_paths_requiring() {
        let mut g = instance1();
        assert_eq_unordered_unique!(g.paths_requiring(edge(&g, 0, 1, Eq)), []);
        assert_eq_unordered_unique!(g.paths_requiring(edge(&g, 0, 1, Neq)), []);
        assert_eq_unordered_unique!(
            g.paths_requiring(edge(&g, 1, 2, Eq)),
            [
                path(&g, 1, 2, Eq),
                path(&g, 0, 2, Eq),
                path(&g, 0, 4, Neq),
                path(&g, 1, 4, Neq),
                path(&g, 5, 2, Eq),
                path(&g, 5, 4, Neq),
                path(&g, 6, 2, Neq)
            ]
        );
        assert_eq_unordered_unique!(
            g.paths_requiring(edge(&g, 2, 1, Eq)),
            [
                path(&g, 2, 1, Eq),
                path(&g, 2, 2, Neq),
                path(&g, 2, 5, Eq),
                path(&g, 2, 6, Eq),
                path(&g, 2, 0, Eq),
                path(&g, 2, 0, Neq),
                path(&g, 2, 3, Eq),
                path(&g, 2, 1, Neq),
                path(&g, 2, 3, Neq),
                path(&g, 2, 5, Neq),
                path(&g, 2, 6, Neq),
            ]
        );
        g.insert_node(Node::Val(7));
        g.add_edge(edge(&g, 4, 7, Eq));
        assert_eq_unordered_unique!(
            g.paths_requiring(edge(&g, 7, 4, Neq)),
            [path(&g, 7, 4, Neq), path(&g, 7, 7, Neq), path(&g, 4, 4, Neq)]
        );
    }
}
