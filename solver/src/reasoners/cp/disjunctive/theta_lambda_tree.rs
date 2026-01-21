#![allow(unused)]

use std::ops::{BitXor, Index, IndexMut, Range};

use itertools::Itertools;

use crate::core::IntCst;

type ActivityId = usize;

const EMPTY_DUR: IntCst = 0;
const EMPTY_ECT: IntCst = 0;

#[derive(Default, Debug, Copy, Clone)]
pub struct Activity {
    id: ActivityId,
    est: IntCst,
    lct: IntCst,
    p: IntCst,
    optional: bool,
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
struct TLNode {
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
pub struct TLTree {
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

    fn clear_tree(&mut self) {
        for node in &mut self.tree {
            *node = TLNode::EMPTY
        }
    }

    fn task_to_node(&self, task: usize) -> Node {
        // first task is at (capacity -1)
        Node(self.capacity - 1 + task)
    }

    /// If this node is a leaf, returns the corresponding task index
    fn node_to_task(&self, node: Node) -> Option<usize> {
        if node.0 >= self.capacity - 1 {
            Some(node.0 - self.capacity + 1)
        } else {
            None
        }
    }

    fn tasks(&self) -> Range<usize> {
        0..self.activities.len()
    }

    fn in_tree(&self, task: usize) -> bool {
        !self[self.task_to_node(task)].is_empty()
    }

    pub fn insert(&mut self, task: usize) {
        let node = self.task_to_node(task);
        let task = &self.activities[task];
        self[node] = task.tree_node();
        self.propagate_update(node);
    }

    pub fn remove(&mut self, task: usize) {
        let node = self.task_to_node(task);
        self[node] = TLNode::EMPTY;
        self.propagate_update(node);
    }

    pub fn in_tree_activities(&self) -> impl Iterator<Item = &Activity> + '_ {
        (0..self.activities.len()).filter_map(|tid| {
            let n = self.task_to_node(tid);
            if !self[n].is_empty() {
                Some(&self.activities[tid])
            } else {
                None
            }
        })
    }

    pub fn theta(&self) -> impl Iterator<Item = &Activity> + '_ {
        self.in_tree_activities().filter(|a| !a.optional)
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
        self.tree[0].ect
    }
    pub fn ect_theta_lambda(&self) -> IntCst {
        self.tree[0].ect_opt
    }

    /// Returns the Latest Completion Time (LCT) of activities in the tree.
    ///
    /// Complexity: `O(N)`
    pub fn lct_theta(&self) -> IntCst {
        self.theta().map(|a| a.lct).max().unwrap()
    }

    /// Returns the Earliest Starting Time (EST) of activities in the tree.
    ///
    /// Complexity: `O(N)`
    pub fn est_theta(&self) -> IntCst {
        self.theta().map(|a| a.est).min().unwrap()
    }

    /// Look for all subsets of activities if there is an overloaded one.
    /// If there is, the method returns true and the tree will contain an overloaded subset.
    pub fn find_overloaded_subset(&mut self) -> bool {
        self.clear_tree();

        let num_activities = self.activities.len();
        let acts = (0..num_activities)
            .sorted_by_cached_key(|a| self.activities[*a].lct)
            .collect_vec();

        for j in acts {
            self.insert(j);
            debug_assert!(self.in_tree_activities().count() > 0);
            debug_assert!(self.lct_theta() >= self.activities[j].lct);
            if self.ect_theta() > self.activities[j].lct {
                debug_assert!(self.is_overloaded());
                return true;
            } else {
                debug_assert!(!self.is_overloaded())
            }
        }

        false
    }

    /// Returns true if the tree is currently overloaded.
    pub fn is_overloaded(&self) -> bool {
        self.ect_theta() > self.lct_theta()
    }

    /// Given an already overloaded tree, select a minimal subset that is still overloaded
    pub fn minimize_overloaded_set(&mut self) {
        debug_assert!(self.is_overloaded());
        for task in self.tasks() {
            debug_assert!(self.is_overloaded());
            if !self.in_tree(task) {
                continue;
            }
            self.remove(task);
            if !self.is_overloaded() {
                self.insert(task);
            }
        }
        debug_assert!(self.is_overloaded());
    }

    /// Explains why the theta tree is currently overloaded
    pub fn explain_overload(&mut self) -> Explanation {
        let est_theta = self.est_theta();
        let lct_theta = self.lct_theta();

        let mut explanation = Vec::new();
        debug_assert!(self.is_overloaded());
        for task in self.in_tree_activities() {
            explanation.push(ExplanationItem::EstGeq(task.id, est_theta));
            explanation.push(ExplanationItem::DurationGeq(task.id, task.p));
            explanation.push(ExplanationItem::LctLeq(task.id, lct_theta));
        }
        explanation
    }
    /// Look for all subsets of activities if there is an overloaded one.
    /// If there is, the method returns true and the tree will contain an overloaded subset.
    pub fn check_overload(&mut self) -> bool {
        self.clear_tree();

        let num_activities = self.activities.len();
        let mut acts = (0..num_activities).map(|a| (a, self.activities[a].lct)).collect_vec();
        acts.sort_unstable_by_key(|(_a, lct)| *lct);

        for (i, lct_i) in acts {
            self.insert(i);
            debug_assert!(self.in_tree_activities().count() > 0);
            if !self.activities[i].optional {
                debug_assert!(self.lct_theta() >= lct_i);

                if self.ect_theta() > lct_i {
                    // overloaded based on the present (white) nodes only.
                    println!("OVERLOADED");
                    debug_assert!(self.is_overloaded());
                    return true;
                }
                while self.ect_theta_lambda() > lct_i {
                    // there is a grey node that, if added, would cause an overload
                    // this task is the one that participates in the computation of ECT(Theta, Lambda)
                    let opt_overloader = self.cause_of_ect_theta_lambda(Node::ROOT);
                    println!("overloader: {opt_overloader} {:?}", self.activities[opt_overloader]);
                    // restore feasibility by forcing its absence and removing it from the tree
                    self.remove(opt_overloader);
                    // TODO: deactivate
                }
            }
        }

        false
    }
    fn cause_of_ect_theta_lambda(&self, node: Node) -> ActivityId {
        if let Some(task) = self.node_to_task(node) {
            // we have reached a leaf, this must be the culprit
            debug_assert!(self.activities[task].optional);
            return task;
        }
        let n = self[node];
        debug_assert_ne!(n.ect, n.ect_opt, "no culprit");
        let left = self[node.left_child()];
        let right = self[node.right_child()];
        if n.ect_opt == right.ect_opt {
            self.cause_of_ect_theta_lambda(node.right_child())
        } else if n.ect_opt == left.ect + right.sum_p_opt {
            self.cause_of_sum_p_theta_lambda(node.right_child())
        } else if n.ect_opt == left.ect_opt + right.sum_p {
            self.cause_of_ect_theta_lambda(node.left_child())
        } else {
            unreachable!()
        }
    }
    fn cause_of_sum_p_theta_lambda(&self, node: Node) -> ActivityId {
        if let Some(task) = self.node_to_task(node) {
            // we have reached a leaf, this must be the culprit
            debug_assert!(self.activities[task].optional);
            return task;
        }
        let n = self[node];
        debug_assert_ne!(n.sum_p, n.sum_p_opt, "no culprit");
        let left = self[node.left_child()];
        let right = self[node.right_child()];
        if n.sum_p_opt == left.sum_p + right.sum_p_opt {
            self.cause_of_sum_p_theta_lambda(node.right_child())
        } else if n.sum_p_opt == left.sum_p_opt + right.sum_p {
            self.cause_of_sum_p_theta_lambda(node.left_child())
        } else {
            unreachable!()
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

pub(crate) type Explanation = Vec<ExplanationItem>;
pub(crate) enum ExplanationItem {
    EstGeq(ActivityId, IntCst),
    DurationGeq(ActivityId, IntCst),
    LctLeq(ActivityId, IntCst),
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

    #[test]
    fn test() {
        let activities = vec![
            Activity::new(0, 0, 20, 5, false),
            Activity::new(1, 25, 60, 9, false),
            Activity::new(2, 30, 60, 5, true),
            Activity::new(3, 32, 47, 10, false),
        ];

        let mut tt = TLTree::init_empty(activities);

        println!("{:?}", tt);
        tt.display();

        tt.insert(0);
        tt.display();
        tt.insert(1);
        tt.display();
        tt.insert(2);
        tt.display();
        tt.insert(3);
        tt.display();

        assert!(!tt.find_overloaded_subset())
    }
    #[test]
    fn test_overload() {
        let overloaded = vec![
            vec![
                Activity::new(2, 30, 35, 4, false),
                Activity::new(1, 35, 41, 6, false),
                Activity::new(3, 32, 47, 10, false),
            ],
            vec![
                Activity::new(0, 0, 6, 5, false),
                Activity::new(2, 30, 35, 4, false),
                Activity::new(1, 5, 40, 6, false),
                Activity::new(3, 32, 43, 10, false),
            ],
        ];
        let not_overloaded = vec![
            vec![
                Activity::new(2, 30, 35, 4, false),
                Activity::new(1, 5, 40, 6, false),
                Activity::new(3, 32, 50, 10, false),
            ],
            vec![
                Activity::new(0, 0, 6, 5, false),
                Activity::new(2, 30, 35, 4, false),
                Activity::new(1, 5, 40, 6, false),
                Activity::new(3, 32, 47, 10, false),
            ],
        ];

        for acts in overloaded {
            println!("{acts:?}");
            let mut tt = TLTree::init_empty(acts);
            assert!(tt.find_overloaded_subset());
            assert!(tt.is_overloaded());
            tt.minimize_overloaded_set();
            println!("Minimized set:");
            tt.display();
        }

        for acts in not_overloaded {
            println!("{acts:?}");
            let mut tt = TLTree::init_empty(acts);
            assert!(!tt.find_overloaded_subset())
        }
    }
}
