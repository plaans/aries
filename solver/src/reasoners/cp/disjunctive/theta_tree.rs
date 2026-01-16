use std::ops::{BitXor, Index, IndexMut, Range};

use itertools::Itertools;

use crate::core::IntCst;

type ActivityId = usize;

#[derive(Default, Debug, Copy, Clone)]
pub struct Activity {
    id: ActivityId,
    est: IntCst,
    lct: IntCst,
    p: IntCst,
}

impl Activity {
    pub fn new(id: ActivityId, est: IntCst, lct: IntCst, p: IntCst) -> Self {
        Activity { id, est, lct, p }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq)]
struct ThetaNode {
    sum_p: IntCst,
    ect: IntCst,
}

impl ThetaNode {
    pub fn is_empty(&self) -> bool {
        self == &ThetaNode::default()
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
pub struct ThetaTree {
    activities: Vec<Activity>,
    tree: Vec<ThetaNode>,
    capacity: usize,
}

impl ThetaTree {
    pub fn init_empty(mut activities: Vec<Activity>) -> Self {
        activities.sort_unstable_by_key(|a| a.est);
        let capacity = activities.len().next_power_of_two();
        let tree_size = capacity * 2 - 1;
        let tree = vec![ThetaNode::default(); tree_size];

        ThetaTree {
            activities,
            tree,
            capacity,
        }
    }

    fn clear_tree(&mut self) {
        for node in &mut self.tree {
            *node = ThetaNode::default()
        }
    }

    fn task_to_node(&self, task: usize) -> Node {
        // first task is at (capacity -1)
        Node(self.capacity - 1 + task)
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
        self[node] = ThetaNode {
            sum_p: task.p,
            ect: task.est + task.p,
        };
        self.propagate_update(node);
    }

    pub fn remove(&mut self, task: usize) {
        let node = self.task_to_node(task);
        self[node] = ThetaNode::default();
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
        self[n] = ThetaNode {
            sum_p: left.sum_p + right.sum_p,
            ect: IntCst::max(right.ect, left.ect + right.sum_p),
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

    /// Returns the Latest Completion Time (LCT) of activities in the tree.
    ///
    /// Complexity: `O(N)`
    pub fn lct_theta(&self) -> IntCst {
        self.in_tree_activities().map(|a| a.lct).max().unwrap()
    }

    /// Returns the Earliest Starting Time (EST) of activities in the tree.
    ///
    /// Complexity: `O(N)`
    pub fn est_theta(&self) -> IntCst {
        self.in_tree_activities().map(|a| a.est).min().unwrap()
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
}

impl Index<Node> for ThetaTree {
    type Output = ThetaNode;

    fn index(&self, index: Node) -> &Self::Output {
        &self.tree[index.0]
    }
}
impl IndexMut<Node> for ThetaTree {
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
            //Activity::new(0, 0, 6, 5),
            Activity::new(2, 30, 35, 4),
            Activity::new(1, 5, 40, 6),
            Activity::new(3, 32, 47, 10),
        ];

        let mut tt = ThetaTree::init_empty(activities);

        println!("{:?}", tt);
        tt.display();

        tt.insert(0);
        tt.display();
        tt.insert(1);
        tt.display();
        // tt.insert(2);
        // tt.display();
        // tt.insert(3);
        // tt.display();

        assert!(!tt.find_overloaded_subset())
    }
    #[test]
    fn test_overload() {
        let overloaded = vec![
            vec![
                Activity::new(2, 30, 35, 4),
                Activity::new(1, 35, 41, 6),
                Activity::new(3, 32, 47, 10),
            ],
            vec![
                Activity::new(0, 0, 6, 5),
                Activity::new(2, 30, 35, 4),
                Activity::new(1, 5, 40, 6),
                Activity::new(3, 32, 43, 10),
            ],
        ];
        let not_overloaded = vec![
            vec![
                Activity::new(2, 30, 35, 4),
                Activity::new(1, 5, 40, 6),
                Activity::new(3, 32, 50, 10),
            ],
            vec![
                Activity::new(0, 0, 6, 5),
                Activity::new(2, 30, 35, 4),
                Activity::new(1, 5, 40, 6),
                Activity::new(3, 32, 47, 10),
            ],
        ];

        for acts in overloaded {
            println!("{acts:?}");
            let mut tt = ThetaTree::init_empty(acts);
            assert!(tt.find_overloaded_subset());
            assert!(tt.is_overloaded());
            tt.minimize_overloaded_set();
            println!("Minimized set:");
            tt.display();
        }

        for acts in not_overloaded {
            println!("{acts:?}");
            let mut tt = ThetaTree::init_empty(acts);
            assert!(!tt.find_overloaded_subset())
        }
    }
}
