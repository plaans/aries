use std::ops::{BitXor, Index, IndexMut};

use crate::core::{INT_CST_MAX, IntCst};

/// External identifier of an activity
type ActivityId = usize;

/// Task identifier: Index in the internal activity array of the tree (typically ordered by EST)
#[derive(Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub struct Task(u32);

const EMPTY_DUR: IntCst = 0;
const EMPTY_ECT: IntCst = 0;

#[derive(Default, Debug, Copy, Clone)]
pub struct Activity {
    pub id: ActivityId,
    pub est: IntCst,
    pub lct: IntCst,
    pub p: IntCst,
    pub optional: bool,
}

impl Activity {
    pub fn new(id: ActivityId, est: IntCst, lct: IntCst, p: IntCst, optional: bool) -> Self {
        Activity {
            id,
            est,
            lct,
            p,
            optional,
        }
    }

    fn tree_node(&self) -> TLNode {
        let dur = self.p;
        let ect = self.est + dur;
        TLNode {
            sum_p: if self.optional { EMPTY_DUR } else { dur },
            ect: if self.optional { EMPTY_ECT } else { self.est + self.p },
            sum_p_opt: dur,
            ect_opt: ect,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) struct TLNode {
    /// Sum of the duration of the present (white) nodes underneath
    sum_p: IntCst,
    /// Earliest completion time of the present (white) nodes underneath
    ect: IntCst,
    /// Sum of the duration of the present (white) nodes underneath + the duration of the longest optional (grey) one
    sum_p_opt: IntCst,
    /// Earliest completion of all while nodes and one grey node (the grey node is the one inducing the largest ECT)
    ect_opt: IntCst,
}

impl TLNode {
    const EMPTY: TLNode = TLNode {
        sum_p: EMPTY_DUR,
        ect: EMPTY_ECT,
        sum_p_opt: EMPTY_DUR,
        ect_opt: EMPTY_ECT,
    };
    pub fn is_empty(&self) -> bool {
        self == &TLNode::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
struct Node(usize);

impl Node {
    pub const ROOT: Node = Node(0);

    pub fn parent(self) -> Self {
        Node((self.0 - 1) / 2)
    }

    pub fn left_child(self) -> Self {
        Node(self.0 * 2 + 1)
    }
    pub fn right_child(self) -> Self {
        Node(self.0 * 2 + 2)
    }

    #[allow(unused)]
    pub fn sibling(self) -> Self {
        Node((self.0 + 1).bitxor(1) - 1)
    }
}

#[derive(Default, Debug)]
pub(super) struct TLTree {
    activities: Vec<Activity>,
    tree: Vec<TLNode>,
    capacity: usize,
}

impl TLTree {
    pub fn init_empty(mut activities: Vec<Activity>) -> Self {
        activities.sort_unstable_by_key(|a| a.est);
        let capacity = activities.len().next_power_of_two();
        let tree_size = capacity * 2 - 1;
        let tree = vec![TLNode::EMPTY; tree_size];

        TLTree {
            activities,
            tree,
            capacity,
        }
    }

    #[allow(unused)]
    pub fn clear_tree(&mut self) {
        for node in &mut self.tree {
            *node = TLNode::EMPTY
        }
    }

    /// Maps a task index into the corresponding node of the tree (guaranteed to be a leaf)
    fn task_to_node(&self, task: Task) -> Node {
        // first task is at (capacity -1)
        Node(self.capacity - 1 + task.0 as usize)
    }

    /// If this node is a leaf, returns the corresponding task index
    fn node_to_task(&self, node: Node) -> Option<Task> {
        if node.0 >= self.capacity - 1 {
            Some(Task((node.0 + 1 - self.capacity) as u32))
        } else {
            None
        }
    }

    /// All tasks considered, even if they have not be inserted in the tree yet.
    ///
    /// Task are sorted by increasing EST
    pub fn tasks(&self) -> impl Iterator<Item = Task> + use<> {
        (0..self.activities.len()).map(|i| Task(i as u32))
    }

    pub fn task(&self, task: Task) -> &Activity {
        &self.activities[task.0 as usize]
    }

    pub fn insert(&mut self, task: Task) {
        let node = self.task_to_node(task);
        let task = &self.task(task);
        self[node] = task.tree_node();
        self.propagate_update(node);
    }

    pub fn remove(&mut self, task: Task) {
        let node = self.task_to_node(task);
        self[node] = TLNode::EMPTY;
        self.propagate_update(node);
    }

    fn in_tree_activities(&self) -> impl Iterator<Item = &Activity> + '_ {
        self.tasks().filter_map(|tid| {
            let n = self.task_to_node(tid);
            if !self[n].is_empty() {
                Some(self.task(tid))
            } else {
                None
            }
        })
    }

    fn theta(&self) -> impl Iterator<Item = &Activity> + '_ {
        self.in_tree_activities().filter(|a| !a.optional) // FIXME: should be white nodes (in edge finding grey != optional)
    }

    /// After an update of `node`, recomputes the update of all nodes to the root
    fn propagate_update(&mut self, mut node: Node) {
        while node != Node::ROOT {
            node = node.parent();
            self.recompute(node);
        }
    }
    fn recompute(&mut self, n: Node) {
        let left = self[n.left_child()];
        let right = self[n.right_child()];
        self[n] = TLNode {
            sum_p: left.sum_p + right.sum_p,
            ect: IntCst::max(right.ect, left.ect + right.sum_p),
            sum_p_opt: IntCst::max(left.sum_p_opt + right.sum_p, left.sum_p + right.sum_p_opt),
            ect_opt: right
                .ect_opt
                .max(left.ect + right.sum_p_opt)
                .max(left.ect_opt + right.sum_p),
        }
    }

    #[allow(unused)]
    pub fn display(&self) {
        println!("LCT_THETA: {}", self.lct_theta());
        println!("LCT_THETA_LAMBDA: {}", self.lct_theta_lambda());
        self.print(Node::ROOT, 0);
    }

    fn print(&self, node: Node, depth: usize) {
        if node.0 < self.tree.len() {
            println!("{}{:?}", "    ".repeat(depth), &self[node]);
            self.print(node.left_child(), depth + 1);
            self.print(node.right_child(), depth + 1);
        }
    }

    pub fn ect_theta(&self) -> IntCst {
        self[Node::ROOT].ect
    }
    pub fn ect_theta_lambda(&self) -> IntCst {
        self[Node::ROOT].ect_opt
    }

    /// Returns the Latest Completion Time (LCT) of activities in the tree (white nodes only).
    ///
    /// Complexity: `O(N)`
    fn lct_theta(&self) -> IntCst {
        self.theta().map(|a| a.lct).max().unwrap_or(INT_CST_MAX)
    }
    /// Returns the Latest Completion Time (LCT) of activities in the tree (white nodes + one grey node).
    ///
    /// Complexity: `O(N)`
    fn lct_theta_lambda(&self) -> IntCst {
        self.in_tree_activities().map(|a| a.lct).max().unwrap_or(INT_CST_MAX)
    }

    /// Returns the Earliest Starting Time (EST) that cause the current value of [`Node::ect`]
    ///
    /// Complexity: `O(log(N))`
    pub fn est_theta(&self) -> IntCst {
        self._est_theta(Node::ROOT)
    }

    /// Returns the Earliest Starting Time (EST) that cause the current value of [`Node::ect`]
    ///
    /// Complexity: `O(log(N))`
    fn _est_theta(&self, node: Node) -> IntCst {
        if let Some(task) = self.node_to_task(node) {
            // we are at a leaf, return the est
            return self.task(task).est;
        }
        let right = node.right_child();
        let left = node.left_child();
        let next = if self[node].ect == self[right].ect {
            // ect was set from the right chlid, so, est comes from there as well
            right
        } else {
            // ect must come from left
            debug_assert_eq!(self[node].ect, self[left].ect + self[right].sum_p);
            left
        };
        self._est_theta(next) // tail recursive, hoping it will bo optimized into a loop
    }
    /// Returns the Earliest Starting Time (EST) that causes the current value of [`Node::ect_opt`]
    ///
    /// Complexity: `O(log(N))`
    pub fn est_theta_lambda(&self) -> IntCst {
        self._est_theta_lambda(Node::ROOT)
    }
    fn _est_theta_lambda(&self, node: Node) -> IntCst {
        if let Some(task) = self.node_to_task(node) {
            // we are at a leaf, return the est
            return self.task(task).est;
        }
        let right = node.right_child();
        let left = node.left_child();
        if self[node].ect_opt == self[left].ect + self[right].sum_p_opt {
            // we are looking of the one causing `left.ect` (NOT left.ect_opt !)
            return self._est_theta(left);
        }
        let next = if self[node].ect_opt == self[right].ect_opt {
            // ect_opt was set from the right chlid, so, est comes from there as well
            right
        } else {
            // ect_opt must come from left (there are two possibilities for that)
            debug_assert!(self[node].ect_opt == self[left].ect_opt + self[right].sum_p);
            left
        };
        self._est_theta_lambda(next) // tail recursive, hoping it will be optimized into a loop
    }

    /// Returns the grey task that participates in the current value of ECT(Theta, Lambda)
    pub fn cause_of_ect_theta_lambda(&self) -> Task {
        self._cause_of_ect_theta_lambda(Node::ROOT)
    }
    fn _cause_of_ect_theta_lambda(&self, node: Node) -> Task {
        if let Some(task) = self.node_to_task(node) {
            // we have reached a leaf, this must be the culprit
            debug_assert!(self.task(task).optional);
            return task;
        }
        let n = self[node];
        debug_assert_ne!(n.ect, n.ect_opt, "no culprit");
        let left = self[node.left_child()];
        let right = self[node.right_child()];
        if n.ect_opt == right.ect_opt {
            self._cause_of_ect_theta_lambda(node.right_child())
        } else if n.ect_opt == left.ect + right.sum_p_opt {
            self.cause_of_sum_p_theta_lambda(node.right_child())
        } else {
            debug_assert!(n.ect_opt == left.ect_opt + right.sum_p);
            self._cause_of_ect_theta_lambda(node.left_child())
        }
    }
    fn cause_of_sum_p_theta_lambda(&self, node: Node) -> Task {
        if let Some(task) = self.node_to_task(node) {
            // we have reached a leaf, this must be the culprit
            debug_assert!(self.task(task).optional);
            return task;
        }
        let n = self[node];
        debug_assert_ne!(n.sum_p, n.sum_p_opt, "no culprit");
        let left = self[node.left_child()];
        let right = self[node.right_child()];
        if n.sum_p_opt == left.sum_p + right.sum_p_opt {
            self.cause_of_sum_p_theta_lambda(node.right_child())
        } else {
            debug_assert!(n.sum_p_opt == left.sum_p_opt + right.sum_p);
            self.cause_of_sum_p_theta_lambda(node.left_child())
        }
    }
}

impl Index<Node> for TLTree {
    type Output = TLNode;

    fn index(&self, index: Node) -> &Self::Output {
        &self.tree[index.0]
    }
}
impl IndexMut<Node> for TLTree {
    fn index_mut(&mut self, index: Node) -> &mut Self::Output {
        &mut self.tree[index.0]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_nodes() {
        for i in 0..20usize {
            let n = Node(i);
            assert_eq!(n.left_child().parent(), n);
            assert_eq!(n.right_child().parent(), n);
            assert_eq!(n.left_child().sibling(), n.right_child());
            assert_eq!(n.right_child().sibling(), n.left_child());
        }
    }
}
