#![allow(unused)]

use std::{
    cell::RefCell,
    collections::HashSet,
    ops::{BitXor, Index, IndexMut, Range},
};

use itertools::Itertools;

use crate::core::{INT_CST_MAX, IntCst};

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

use super::theta_tree::Node;

#[derive(Default, Debug)]
pub(super) struct TLTree {
    activities: Vec<Activity>,
    tree: Vec<TLNode>,
    capacity: usize,
    /// An internal buffer used to compute and share explanations and inferences
    buffer: Vec<ExplanationItem>,
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
            buffer: Default::default(),
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
            Some(node.0 + 1 - self.capacity)
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
    pub fn lambda(&self) -> impl Iterator<Item = &Activity> + '_ {
        self.in_tree_activities().filter(|a| a.optional)
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
        self.tree[0].ect
    }
    pub fn ect_theta_lambda(&self) -> IntCst {
        self.tree[0].ect_opt
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
    fn est_theta(&self, node: Node) -> IntCst {
        if let Some(task) = self.node_to_task(node) {
            // we are at a leaf, return the est
            return self.activities[task].est;
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
        self.est_theta(next) // tail recursive, hoping it will bo optimized into a loop
    }
    /// Returns the Earliest Starting Time (EST) that causes the current value of [`Node::ect_opt`]
    ///
    /// Complexity: `O(log(N))`
    fn est_theta_lambda(&self, node: Node) -> IntCst {
        if let Some(task) = self.node_to_task(node) {
            // we are at a leaf, return the est
            return self.activities[task].est;
        }
        let right = node.right_child();
        let left = node.left_child();
        if self[node].ect_opt == self[left].ect + self[right].sum_p_opt {
            // we are looking of the one causing `left.ect` (NOT left.ect_opt !)
            return self.est_theta(left);
        }
        let next = if self[node].ect_opt == self[right].ect_opt {
            // ect_opt was set from the right chlid, so, est comes from there as well
            right
        } else {
            // ect_opt must come from left (there are two possibilities for that)
            debug_assert!(self[node].ect_opt == self[left].ect_opt + self[right].sum_p);
            left
        };
        self.est_theta_lambda(next) // tail recursive, hoping it will be optimized into a loop
    }

    /// Look for all subsets of activities if there is an overloaded one.
    /// If there is, the method returns true and the tree will contain an overloaded subset.
    fn find_overloaded_subset(&mut self) -> bool {
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
    fn is_overloaded(&self) -> bool {
        self.ect_theta() > self.lct_theta()
    }

    /// Returns true if the tree would be overloaded if one optional task form lambda was to be added
    fn is_opt_overloaded(&self) -> bool {
        self.ect_theta_lambda() > self.lct_theta()
    }

    /// Explains why the theta tree is currently overloaded
    fn explain_overload(&mut self, out: &mut Vec<ExplanationItem>) {
        out.clear();
        let est_theta = self.est_theta(Node::ROOT);
        let lct_theta = self.lct_theta();
        debug_assert!(self.ect_theta() > lct_theta);
        let mut sum_duration = 0;

        debug_assert!(self.is_overloaded());
        for task in self.theta() {
            if task.est < est_theta {
                continue;
            }
            out.push(ExplanationItem::Present(task.id));
            out.push(ExplanationItem::EstGeq(task.id, est_theta));
            out.push(ExplanationItem::DurationGeq(task.id, task.p));
            out.push(ExplanationItem::LctLeq(task.id, lct_theta));
            sum_duration += task.p;
        }
        debug_assert_eq!(est_theta + sum_duration, self.ect_theta());
    }
    /// Look for all subsets of activities if there is an overloaded one.
    /// If there is, the method returns true and the tree will contain an overloaded subset.
    pub fn check_overload<'a>(&mut self, buffer: &'a mut Vec<ExplanationItem>) -> PropagationResult<'a> {
        let res = self._check_overload(buffer);
        #[cfg(debug_assertions)]
        self.verify_propagation_result(res.clone());
        res
    }

    /// Implements [`Self::check_overload`], but without the validation step at the end (to avoid recursive invocation in the tests)
    fn _check_overload<'a>(&mut self, buffer: &'a mut Vec<ExplanationItem>) -> PropagationResult<'a> {
        self.clear_tree();
        buffer.clear();

        let num_activities = self.activities.len();
        let mut acts = (0..num_activities).map(|a| (a, self.activities[a].lct)).collect_vec();
        acts.sort_unstable_by_key(|(_a, lct)| *lct);

        for (i, lct_i) in acts {
            self.insert(i);
            debug_assert!(self.in_tree_activities().count() > 0);

            if self.ect_theta() > lct_i {
                // overloaded based on the present (white) nodes only.
                self.explain_overload(buffer);
                return PropagationResult::Conflict(buffer);
            }
            while self.ect_theta_lambda() > lct_i {
                // there is a grey node that, if added, would cause an overload
                // this task is the one that participates in the computation of ECT(Theta, Lambda)
                let opt_overloader = self.cause_of_ect_theta_lambda(Node::ROOT);
                // restore feasibility by forcing its absence and removing it from the tree
                self.remove(opt_overloader);
                buffer.push(ExplanationItem::Absent(self.activities[opt_overloader].id));
            }
        }

        PropagationResult::Inferences(buffer)
    }

    /// Knowing that an overload deactivation of an optional task was triggered for the current set of activities,
    /// Returns an activity that would be deactivated together with the cause of the deactivation
    /// (set of literals that, if all true) would force the activity to be absent.
    pub fn explain_overload_deactivation(&mut self) -> (&[ExplanationItem], ActivityId) {
        // there might be more than one optional task in our activities.
        // It is expected that the common case would be having just one because the requester would put us in this situation,
        // leaving only the relevant optional tasks (typically one).
        // Handling the case with only one could be the subject of a fast-track which is not implemented yet.
        //
        // In the general case, we need to find the overload point, thus we reproduce the propagation algorithm

        self.clear_tree();
        let mut buffer = Vec::new();
        std::mem::swap(&mut buffer, &mut self.buffer);

        let num_activities = self.activities.len();
        let mut acts = (0..num_activities).map(|a| (a, self.activities[a].lct)).collect_vec();
        acts.sort_unstable_by_key(|(_a, lct)| *lct);

        let mut culprit = None;
        for (i, lct_i) in acts {
            self.insert(i);
            debug_assert!(self.in_tree_activities().count() > 0);

            debug_assert!(
                self.ect_theta() <= lct_i,
                "Trying to explain overload-deactivation in an already overloaded instance."
            );
            if self.ect_theta_lambda() > lct_i {
                // there is a grey node that, if added, would cause an overload
                // this task is the one that participates in the computation of ECT(Theta, Lambda)
                culprit = Some((self.cause_of_ect_theta_lambda(Node::ROOT), lct_i));
                // we have found the state in which we would propagate: stop iteration,
                break;
            }
        }
        debug_assert!(self.lambda().count() >= 1); // there should be at least one optional task
        debug_assert!(self.is_opt_overloaded());
        let (culprit, lct_tl) = culprit.expect("iteration finished without identifying an activity to deactivate");

        // now we have a minimal set of white nodes that forbid the presence of the culprit
        // so generate the explanation:
        // In a nutshell, the explanation is that we cannot have all tasks within [est_tl, lct_tl] because it smaller than the sum of all their durations.
        let est_tl = self.est_theta_lambda(Node::ROOT);
        // let lct_tl = self.lct_theta();
        let mut sum_duration = 0;
        let culprit_task = self.activities[culprit];
        buffer.push(ExplanationItem::EstGeq(culprit_task.id, est_tl));
        buffer.push(ExplanationItem::DurationGeq(culprit_task.id, culprit_task.p));
        buffer.push(ExplanationItem::LctLeq(culprit_task.id, lct_tl));
        sum_duration += culprit_task.p;
        for task in self.theta() {
            if task.est < est_tl {
                continue;
            }
            // TODO: we could additionaly ignore a set of task which duration does not contribute to the overload
            //       For instance, it might be the case that we are too long by 10 time units but there is an activity with a duration of 2
            //       that could be removed from the set of culprits
            buffer.push(ExplanationItem::Present(task.id));
            buffer.push(ExplanationItem::EstGeq(task.id, est_tl));
            buffer.push(ExplanationItem::DurationGeq(task.id, task.p));
            buffer.push(ExplanationItem::LctLeq(task.id, lct_tl));
            sum_duration += task.p;
        }
        debug_assert_eq!(est_tl + sum_duration, self.ect_theta_lambda());
        debug_assert!(sum_duration > lct_tl - est_tl); // sanity check that we are overloading

        std::mem::swap(&mut self.buffer, &mut buffer);
        debug_assert!(buffer.is_empty() && !self.buffer.is_empty());

        (&self.buffer, culprit_task.id)
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
        } else {
            debug_assert!(n.ect_opt == left.ect_opt + right.sum_p);
            self.cause_of_ect_theta_lambda(node.left_child())
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
        } else {
            debug_assert!(n.sum_p_opt == left.sum_p_opt + right.sum_p);
            self.cause_of_sum_p_theta_lambda(node.left_child())
        }
    }

    /// Costly verification step to check that:
    ///  - every optional activity deactivated indeed overloads the resource
    ///  - every optional activity *not* deactivated would indeed not overload the resource if it was present
    fn verify_propagation_result<'a>(&'a self, res: PropagationResult<'a>) {
        use super::theta_tree as tt;
        // creates the set of all compulsory activities plus 0 or 1 optional activity (marked as compulsory)
        let acts_with = |opt: Option<ActivityId>| {
            self.activities
                .iter()
                .filter(|&a| (!a.optional || Some(a.id) == opt))
                .map(|a| tt::Activity::new(a.id, a.est, a.lct, a.p))
                .collect_vec()
        };
        let overloaded = |acts: Vec<tt::Activity>| {
            let mut tree = tt::ThetaTree::init_empty(acts);
            tree.find_overloaded_subset()
        };
        match res {
            PropagationResult::Conflict(explanation_items) => {
                let tt_acts = acts_with(None);
            }
            PropagationResult::Inferences(explanation_items) => {
                let mut overloading_opts: HashSet<ActivityId> = Default::default();
                for e in explanation_items {
                    let ExplanationItem::Absent(e) = e else { unreachable!() };
                    overloading_opts.insert(*e);
                }

                for &overloader in &overloading_opts {
                    assert!(overloaded(acts_with(Some(overloader))))
                }

                let non_overloading = self
                    .activities
                    .iter()
                    .filter_map(|a| {
                        if overloading_opts.contains(&a.id) {
                            None
                        } else {
                            Some(a.id)
                        }
                    })
                    .collect_vec();
                for non_overloader in non_overloading {
                    assert!(!overloaded(acts_with(Some(non_overloader))));
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum PropagationResult<'a> {
    Conflict(&'a [ExplanationItem]),
    Inferences(&'a [ExplanationItem]),
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

pub(super) use super::theta_tree::ExplanationItem;

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
            tt.display();
        }

        for acts in not_overloaded {
            println!("{acts:?}");
            let mut tt = TLTree::init_empty(acts);
            assert!(!tt.find_overloaded_subset())
        }
    }
}
