use hashbrown::HashMap;

use crate::{
    backtrack::{Backtrack, Trail},
    collections::ref_store::RefVec,
    create_ref_type,
    reasoners::eq_alt::node::Node,
    transitive_conversion,
};
use std::{cell::RefCell, fmt::Debug};

use super::NodeId;

create_ref_type!(GroupId);
// Commenting these lines allows us to check where nodes are treated like groups and vice versa
transitive_conversion!(NodeId, u32, GroupId);
transitive_conversion!(GroupId, u32, NodeId);

impl Debug for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Node {}", self.to_u32())
    }
}

impl Debug for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Group {}", self.to_u32())
    }
}

#[derive(Clone, Debug)]
struct Relations {
    parent: Option<NodeId>,
    next_sibling: Option<NodeId>,
    previous_sibling: Option<NodeId>,
    first_child: Option<NodeId>,
}

impl Relations {
    const DETACHED: Relations = Relations {
        parent: None,
        next_sibling: None,
        previous_sibling: None,
        first_child: None,
    };
}

impl Default for Relations {
    fn default() -> Self {
        Self::DETACHED
    }
}

/// NodeStore is a backtrackable Id => Node map with Union-Find and path-flattening
#[derive(Clone, Default)]
pub struct NodeStore {
    /// Maps NodeId to Node, doesn't support arbitrary removal!
    nodes: RefVec<NodeId, Node>,
    /// Maps Node to NodeId, for interfacing with the graph
    rev_nodes: HashMap<Node, NodeId>,
    /// Relations between elements of a group of nodes
    group_relations: RefCell<RefVec<NodeId, Relations>>,
    trail: RefCell<Trail<Event>>,
    path: RefCell<Vec<NodeId>>,
}

#[allow(unused)]
impl NodeStore {
    pub fn new() -> NodeStore {
        Default::default()
    }

    pub fn insert_node(&mut self, node: Node) -> NodeId {
        debug_assert!(!self.rev_nodes.contains_key(&node));
        self.trail.borrow_mut().push(Event::Added);
        let id = self.nodes.push(node);
        self.rev_nodes.insert(node, id);
        self.group_relations.borrow_mut().push(Default::default());
        id
    }

    pub fn get_id(&self, node: &Node) -> Option<NodeId> {
        self.rev_nodes.get(node).copied()
    }

    pub fn get_node(&self, id: NodeId) -> Node {
        self.nodes[id]
    }

    pub fn merge_nodes(&mut self, child: NodeId, parent: NodeId) {
        let child = self.get_group_id(child);
        let parent = self.get_group_id(parent);
        self.merge(child, parent);
    }

    pub fn merge(&mut self, child: GroupId, parent: GroupId) {
        debug_assert_eq!(child, self.get_group_id(child.into()));
        debug_assert_eq!(parent, self.get_group_id(parent.into()));
        if child != parent {
            self.set_new_parent(child.into(), parent.into());
        }
    }

    fn set_new_parent(&mut self, id: NodeId, parent_id: NodeId) {
        debug_assert_ne!(id, parent_id);
        // Ensure child has no relations or no parent
        debug_assert!(self.group_relations.borrow()[id].parent.is_none());
        self.reparent(id, parent_id);
    }

    fn reparent(&self, id: NodeId, parent_id: NodeId) {
        debug_assert_ne!(id, parent_id);
        // Get info about node's old status
        let old_relations = { self.group_relations.borrow()[id].clone() };
        self.trail.borrow_mut().push(Event::ParentChanged {
            id,
            old_parent_id: old_relations.parent,
            old_previous_sibling_id: old_relations.previous_sibling,
            old_next_sibling_id: old_relations.next_sibling,
        });

        let mut group_relations_mut = self.group_relations.borrow_mut();

        // If first child, set next sibling as first child
        if let Some(old_parent) = old_relations.parent {
            if old_relations.previous_sibling.is_none() {
                group_relations_mut[old_parent].first_child = old_relations.next_sibling;
            }
        }

        // Join siblings together
        if let Some(old_previous_sibling) = old_relations.previous_sibling {
            group_relations_mut[old_previous_sibling].next_sibling = old_relations.next_sibling;
        }
        if let Some(old_next_sibling) = old_relations.next_sibling {
            group_relations_mut[old_next_sibling].previous_sibling = old_relations.previous_sibling;
        }

        // Set node as first child of new parent
        let parent_relations = &mut group_relations_mut[parent_id];
        let first_sibling = parent_relations.first_child;
        parent_relations.first_child = Some(id);

        // Setup node
        let new_relations = &mut group_relations_mut[id];
        new_relations.previous_sibling = None;
        new_relations.next_sibling = first_sibling;
        new_relations.parent = Some(parent_id);

        if let Some(new_next_sibling) = first_sibling {
            group_relations_mut[new_next_sibling].previous_sibling = Some(id);
        }
    }

    pub fn get_group_id(&self, mut id: NodeId) -> GroupId {
        // Get the path from id to rep (inclusive)
        let mut path = self.path.borrow_mut();
        path.clear();
        path.push(id);
        while let Some(parent_id) = self.group_relations.borrow()[id].parent {
            id = parent_id;
            path.push(id);
        }
        // The rep is the last element
        let rep_id = path.pop().unwrap();

        // The last element doesn't need reparenting
        path.pop();

        for child_id in path.iter() {
            self.reparent(*child_id, rep_id);
        }
        rep_id.into()
    }

    pub fn get_group(&self, id: GroupId) -> Vec<NodeId> {
        let mut res = vec![];

        // Depth first traversal using first_child and next_sibling
        let mut stack = vec![id.into()];
        while let Some(n) = stack.pop() {
            // Visit element in stack
            res.push(n);
            // Starting from first child
            let Some(first_child) = self.group_relations.borrow()[n].first_child else {
                continue;
            };
            stack.push(first_child);
            let gr = self.group_relations.borrow();
            let mut current_relations = &gr[first_child];
            while let Some(next_child) = current_relations.next_sibling {
                stack.push(next_child);
                current_relations = &gr[next_child];
            }
        }
        res
    }

    pub fn get_group_nodes(&self, id: GroupId) -> Vec<Node> {
        self.get_group(id).into_iter().map(|id| self.get_node(id)).collect()
    }

    pub fn groups(&self) -> Vec<GroupId> {
        let relations = self.group_relations.borrow();
        (0..relations.len())
            .filter_map(|i| (relations[i.into()].parent.is_none()).then_some(i.into()))
            .collect()
    }

    pub fn count_groups(&self) -> usize {
        self.groups().len()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn nodes(&self) -> Vec<NodeId> {
        let relations = self.group_relations.borrow();
        (0..relations.len()).map(|i| i.into()).collect()
    }
}

// impl Default for NodeStore {
//     fn default() -> Self {
//         Self::new()
//     }
// }

#[derive(Clone)]
enum Event {
    Added,
    ParentChanged {
        id: NodeId,
        old_parent_id: Option<NodeId>,
        old_previous_sibling_id: Option<NodeId>,
        old_next_sibling_id: Option<NodeId>,
    },
}

impl Backtrack for NodeStore {
    fn save_state(&mut self) -> crate::backtrack::DecLvl {
        self.trail.borrow_mut().save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.borrow_mut().num_saved()
    }

    fn restore_last(&mut self) {
        use Event::*;
        self.trail.borrow_mut().restore_last_with(|e| match e {
            Added => {
                let node = self.nodes.pop().unwrap();
                self.rev_nodes.remove(&node);
                self.group_relations.borrow_mut().pop().unwrap();
            }
            ParentChanged {
                id,
                old_parent_id,
                old_previous_sibling_id,
                old_next_sibling_id,
            } => {
                // NOTE: In this block, "new" refers to the state after the event happened, old before.

                // INVARIANT: Child is first child of it's current parent
                let new_relations = { self.group_relations.borrow()[id].clone() };
                debug_assert_eq!(new_relations.previous_sibling, None);

                let mut group_relations_mut = self.group_relations.borrow_mut();

                if let Some(new_next_sibling) = new_relations.next_sibling {
                    group_relations_mut[new_next_sibling].previous_sibling = None;
                }

                // Set new parent's first child to new next sibling
                group_relations_mut[new_relations.parent.unwrap()].first_child = new_relations.next_sibling;

                let mut_relations = &mut group_relations_mut[id];
                mut_relations.parent = old_parent_id;
                mut_relations.previous_sibling = old_previous_sibling_id;
                mut_relations.next_sibling = old_next_sibling_id;

                if let Some(old_previous_sibling) = old_previous_sibling_id {
                    group_relations_mut[old_previous_sibling].next_sibling = Some(id);
                } else if let Some(old_parent) = old_parent_id {
                    group_relations_mut[old_parent].first_child = Some(id);
                }
                if let Some(old_next_sibling) = old_next_sibling_id {
                    group_relations_mut[old_next_sibling].previous_sibling = Some(id);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_store() {
        use std::collections::HashSet;
        use Node::*;

        let mut ns = NodeStore::new();
        ns.save_state();

        // Insert three distinct nodes
        let n0 = ns.insert_node(Val(0));
        let n1 = ns.insert_node(Val(1));
        let n2 = ns.insert_node(Val(2));

        assert_ne!(ns.get_group_id(n0), ns.get_group_id(n1));
        assert_ne!(ns.get_group_id(n1), ns.get_group_id(n2));

        // Merge n0 and n1, then n1 and n2 => all should be in one group
        ns.merge_nodes(n0, n1);
        ns.merge_nodes(n1, n2);
        let rep = ns.get_group_id(n0);
        assert_eq!(rep, ns.get_group_id(n2));
        assert_eq!(
            ns.get_group(ns.get_group_id(n1)).into_iter().collect::<HashSet<_>>(),
            [n0, n1, n2].into()
        );

        // Merge same nodes again to check idempotency
        ns.merge_nodes(n0, n2);
        assert_eq!(ns.get_group_id(n0), rep);

        // Add a new node and ensure it's separate
        let n3 = ns.insert_node(Val(3));
        assert_ne!(ns.get_group_id(n3), rep);

        ns.save_state();

        // Merge into existing group
        ns.merge_nodes(n2, n3);
        assert_eq!(
            ns.get_group(ns.get_group_id(n3)).into_iter().collect::<HashSet<_>>(),
            [n0, n1, n2, n3].into()
        );

        // Restore to state before n3 was merged
        ns.restore_last();
        assert_ne!(ns.get_group_id(n3), rep);
        assert_eq!(
            ns.get_group(ns.get_group_id(n2)).into_iter().collect::<HashSet<_>>(),
            [n0, n1, n2].into()
        );

        // Restore to initial state
        ns.restore_last();
        assert!(ns.get_id(&Val(0)).is_none());
        assert!(ns.get_id(&Val(1)).is_none());

        // Attempt to query a non-existent node
        assert!(ns.get_id(&Val(99)).is_none());
    }
}
