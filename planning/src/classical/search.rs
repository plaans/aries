use crate::classical::heuristics::*;
use crate::classical::state::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet, VecDeque};
use std::rc::Rc;

/// A node in the search space
/// A node implements a total ordering which is only based on the heuristic value.
struct Node {
    state: State,
    parent: Option<Rc<Node>>,

    /// Step added by this node wrt to the parent node.
    /// To get a complete plan one should accumulate the steps of the ancestors as well
    /// which is done by `Node::extract_plan()`
    steps: Vec<Op>, // TODO: use a small vec

    /// Total plan length (including steps from ancestors)
    plan_length: u32,

    /// A heuristic evaluation of the cost of a solution reachable from this node.
    heuristic: Cost,
}

impl Node {
    pub fn extract_plan(&self) -> Vec<Op> {
        let mut stack = Vec::with_capacity(self.plan_length as usize);
        let mut curr = self;
        for &op in curr.steps.iter().rev() {
            stack.push(op);
        }
        while let Some(next) = &curr.parent {
            curr = next;
            for &op in curr.steps.iter().rev() {
                stack.push(op);
            }
        }
        debug_assert_eq!(stack.len(), self.plan_length as usize);
        stack.reverse();
        stack
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Ordering that prioritizes (in a max heap) nodes with the lowest heuristic value, breaking ties with plan length
impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        Cost::partial_cmp(&other.heuristic, &self.heuristic).unwrap_or_else(|| other.plan_length.cmp(&self.plan_length))
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Node {}

pub struct Cfg {
    // weight given to the heuristic in a weighted A* search
    pub h_weight: Cost,
    // project candidates states down the search tree with lookahead plans
    pub use_lookahead: bool,
}
impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            h_weight: 3.,
            use_lookahead: true,
        }
    }
}

/// Plan search with weighted A* and the h^add heuristic.
///
/// In addition (when enabled in the configuration), for each new node inserted in the open
/// list, the algorithm computes a lookahead plan/node based on a relaxed plan. This generates
/// new node(s) that are added to the open list.
///
/// Returns a solution plan (sequence of operators) that allow to reach a goal state or None
/// if the search space was exhausted without reaching a solution.
///
/// Implementation of [YAHSP2] Alg. 1
pub fn plan_search(initial_state: &State, ops: &Operators, goals: &[Lit], cfg: &Cfg) -> Option<Vec<Op>> {
    let mut heap: BinaryHeap<Rc<Node>> = BinaryHeap::new();
    let mut closed = HashSet::new();

    // initialize the priority queue with the initiale state
    let init = Node {
        state: initial_state.clone(),
        parent: None,
        steps: Vec::new(),
        plan_length: 0,
        heuristic: 0.,
    };
    let insertion_result = compute_node(ops, goals, init, &mut heap, &mut closed, cfg);
    if let Some(solution) = insertion_result {
        return Some(solution.extract_plan());
    }

    // keep expanding the search tree until the priority queue is empty
    while let Some(n) = heap.pop() {
        debug_assert!(
            n.heuristic >= n.plan_length as Cost,
            "The heuristic probably wasn't properly initialized"
        );

        // compute hadd to get the set of applicable actions
        // TODO: there is costly unnecessary work here
        let hres = hadd(&n.state, ops);

        // for each operator applicable in the current state
        for &op in hres.applicable_operators() {
            debug_assert!(n.state.entails_all(ops.preconditions(op)));

            // clone the state and apply effects
            let mut s = n.state.clone();
            s.set_all(ops.effects(op));

            // create the corresponding plan
            let mut plan = n.steps.clone();
            plan.push(op);

            let succ_length = n.plan_length + 1;
            let succ = Node {
                state: s,
                parent: Some(n.clone()),
                steps: vec![op],
                plan_length: succ_length,
                heuristic: 0.,
            };
            // process the node : compute heuristic, insert in open/closed lists
            // also creates probes in the search spaces with looahead plans
            if let Some(solution) = compute_node(ops, goals, succ, &mut heap, &mut closed, cfg) {
                // we have a plan
                return Some(solution.extract_plan());
            }
        }
    }

    // we have exhausted the search space without finding a goal state, problem is unsolvable
    None
}

/// For a given node that is not already in the closed list:
///  - compute its heuristic value
///  - inserts it in the open and closed lists
///  - compute a lookahead plan (that greedily project the node further down the search space)
///  - recursively handle the projected node is recursively handled with the same procedure
///
/// Returns
///  - a solution node if the one passed as argument or one of its projections fulfills the goals
///  - None otherwise  
///
/// Implementation of [YAHSP2] Alg. 2
fn compute_node(
    operators: &Operators,
    goals: &[Lit],
    mut node: Node,
    open: &mut BinaryHeap<Rc<Node>>,
    closed: &mut HashSet<State>,
    cfg: &Cfg,
) -> Option<Node> {
    if closed.contains(&node.state) {
        None
    } else {
        closed.insert(node.state.clone());
        let hres = hadd(&node.state, operators);
        let h_cost = hres.conjunction_cost(goals);
        if h_cost == 0. && node.state.entails_all(goals) {
            Some(node)
        } else if h_cost.is_infinite() {
            None
        } else {
            node.heuristic = node.plan_length as Cost + cfg.h_weight * h_cost;
            let node = Rc::new(node);
            debug_assert!(node.heuristic >= node.plan_length as Cost);
            open.push(node.clone());
            if cfg.use_lookahead {
                let (proj_state, proj_plan) = lookahead(operators, goals, &node.state, &hres);
                let succ_len = node.plan_length + proj_plan.len() as u32;
                let succ = Node {
                    state: proj_state,
                    parent: Some(node),
                    steps: proj_plan,
                    plan_length: succ_len,
                    heuristic: Cost::MIN,
                };
                compute_node(operators, goals, succ, open, closed, cfg)
            } else {
                None
            }
        }
    }
}

/// Extracts a relaxed plan that attemps to reach the goal from the given state
/// The relaxed plan is built in a greedy manner base on the provided operator cost
/// Implementation of ! [YAHSP2] Alg. 5
pub fn extract_relaxed_plan(
    operators: &Operators,
    goals: &[Lit],
    s: &State,
    action_costs: &dyn OperatorCost,
) -> Vec<Op> {
    let mut rplan = Vec::with_capacity(32);
    // todo: use bitset
    let mut satisfied: HashSet<Lit> = s.literals().collect();
    let mut subgoals: VecDeque<Lit> = goals.iter().copied().collect();
    while let Some(g) = subgoals.pop_front() {
        if !satisfied.contains(&g) {
            satisfied.insert(g);
            if let Some((operator, _)) = operators
                .achievers_of(g)
                .iter()
                .map(|&op| (op, action_costs.operator_cost(op)))
                .min_by(|o1, o2| o1.1.partial_cmp(&o2.1).unwrap_or(o1.0.cmp(&o2.0)))
            {
                if !rplan.contains(&operator) {
                    rplan.push(operator);
                    for &cond in operators.preconditions(operator) {
                        subgoals.push_back(cond);
                    }
                }
            } else {
                // no reachable operator for this goal, ignore it
            }
        }
    }
    // comparator to order actions in the relaxed plan.
    // priority is given to
    //  - the operator with lowest cost
    //  - the operator that does does not delete a precondition of the other
    //  - the operator with lowest ID (tie breaking)
    #[allow(clippy::comparison_chain)]
    let cmp = |&a: &Op, &b: &Op| {
        let ca = action_costs.operator_cost(a);
        let cb = action_costs.operator_cost(b);
        if ca < cb {
            Ordering::Less
        } else if ca > cb {
            Ordering::Greater
        } else {
            let a_preconditions = operators.preconditions(a);
            let b_deletes_a = operators.effects(b).iter().any(|&eff| a_preconditions.contains(&!eff));
            let b_preconditions = operators.preconditions(b);
            let a_deletes_b = operators.effects(a).iter().any(|&eff| b_preconditions.contains(&!eff));
            if b_deletes_a && !a_deletes_b {
                Ordering::Less
            } else if a_deletes_b && !b_deletes_a {
                Ordering::Greater
            } else {
                // tie, just make the order deterministic with operator ID
                Op::cmp(&a, &b)
            }
        }
    };
    rplan.sort_by(cmp);
    rplan
}

/// Finds an applicable operators that contributes a precondition of `supported` that `supporter` also provides.
///
/// This function is intended to be used to repair relaxed plan when building a lookahead plan.
/// The logic is detailed in the second part of Algorithm 4 of [YAHSP2]
fn find_applicable_supporting_replacement(
    supporter: Op,
    supported: Op,
    operators: &Operators,
    state: &State,
    op_cost: &impl OperatorCost,
) -> Option<Op> {
    debug_assert!(!state.entails_all(operators.preconditions(supporter)));
    let aj_cond = operators.preconditions(supported);
    // preconditions of aj that are supported by an effect of ai
    operators
        .effects(supporter)
        .iter()
        .filter(|&lit| aj_cond.contains(lit))
        .copied()
        // we have literals : preconditions of `supported` for which `supporter` has an effect
        .flat_map(|lit| {
            // for each candidate literal, retrieve operators that achieve it and have supported preconditions
            operators
                .achievers_of(lit)
                .iter()
                .filter(|&op| state.entails_all(operators.preconditions(*op)))
                .copied()
        })
        // select operator with smallest cost
        .min_by(|&o1, &o2| {
            op_cost
                .operator_cost(o1)
                .partial_cmp(&op_cost.operator_cost(o2))
                .unwrap_or(o1.cmp(&o2))
        })
}

/// Build a lookahead
fn lookahead(
    operators: &Operators,
    goals: &[Lit],
    base_state: &State,
    action_costs: &impl OperatorCost,
) -> (State, Vec<Op>) {
    let mut plan = Vec::with_capacity(32);
    let mut rplan = extract_relaxed_plan(operators, goals, base_state, action_costs);
    let mut s = base_state.clone();

    let mut looping = true;
    while looping {
        looping = false;
        let first_applicable = rplan
            .iter()
            .enumerate()
            .find(|(_, op)| s.entails_all(operators.preconditions(**op)));
        if let Some((index, &op)) = first_applicable {
            looping = true;
            debug_assert!(s.entails_all(operators.preconditions(op)));
            debug_assert!(rplan[index] == op);
            s.set_all(operators.effects(op));
            plan.push(op);
            rplan.remove(index);
        } else {
            // no applicable actions in the relaxed plan, try repairing it
            let mut i = 0;
            let mut j = 0;
            while !looping && i < rplan.len() {
                while !looping && j < rplan.len() {
                    if i == j {
                        // continue
                    } else if let Some(replacement) =
                        find_applicable_supporting_replacement(rplan[i], rplan[j], operators, &s, action_costs)
                    {
                        looping = true;
                        rplan[i] = replacement
                    }
                    j += 1
                }
                i += 1;
            }
        }
    }
    debug_assert!(
        s == plan.iter().fold(base_state.clone(), |mut s, &op| {
            debug_assert!(
                s.entails_all(operators.preconditions(op)),
                "An action of the plan is not applicable in its predecessor state"
            );
            s.set_all(operators.effects(op));
            s
        }),
        "The state resulting in the application of the lookahead plan is not the one returned"
    );
    (s, plan)
}
