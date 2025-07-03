use crate::chronicles::analysis::TemplateID;
use crate::chronicles::constraints::{encode_constraint, Constraint, ConstraintType, Duration, Table};
use crate::chronicles::plan::ActionInstance;
use crate::chronicles::{
    Chronicle, ChronicleLabel, ChronicleTemplate, Condition, Container, Effect, EffectOp, Problem, StateVar, Sub,
    Substitute, Substitution, Time, VarType, TIME_SCALE,
};
use aries::core::state::Term;
use aries::core::{IntCst, Lit, VarRef, INT_CST_MAX};
use aries::model::extensions::partial_assignment::{PartialAssignment, PartialAssignmentBuilder};
use aries::model::lang::linear::LinearSum;
use aries::model::lang::{Atom, Cst, FAtom, IAtom, Rational};
use aries::model::Model;
use aries::solver::Solver;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug)]
struct Transition {
    start: Time,
    end: Time,
    state_var: StateVar,
    pre: Atom,
    post: Atom,
}

/// For the given chronicle template, if it has single transition effect that may be used for rolling,
/// return the corresponding transition and the chronicle without the transition (condition & effect)
/// Returns None if the chronicle is not eligible
fn extract_single_transition(mut ch: ChronicleTemplate) -> Option<(Transition, ChronicleTemplate)> {
    // find the effect to use for the transition
    if ch.chronicle.effects.len() != 1 {
        return None;
    }
    let effect = ch.chronicle.effects.remove(0);
    let EffectOp::Assign(post) = effect.operation else {
        return None;
    };
    // find the condition that matches the effect: instantaneous at the effect start, on the same state variable
    let mut cond_id = 0;
    let condition = loop {
        if cond_id >= ch.chronicle.conditions.len() {
            break None;
        }
        let condition = &ch.chronicle.conditions[cond_id];
        if condition.state_var == effect.state_var
            && condition.start == effect.transition_start
            && condition.end == effect.transition_start
        {
            break Some(ch.chronicle.conditions.remove(cond_id));
        }
        cond_id += 1;
    };
    let condition = condition?;

    // we only consider transitions to and from variables (otherwise, there is no point in rolling the action)
    let is_constant = |atom: Atom| Cst::try_from(atom).is_ok();
    if is_constant(condition.value) || is_constant(post) {
        return None;
    }

    let transition = Transition {
        start: effect.transition_start,
        end: effect.transition_end,
        state_var: effect.state_var,
        pre: condition.value,
        post,
    };
    Some((transition, ch))
}

/// If there is a constant delay between the two timepoints, return it
fn delay(from: Time, to: Time) -> Option<IntCst> {
    if from.denom != to.denom {
        return None;
    }
    if from.num.var != to.num.var {
        return None;
    }
    Some(to.num.shift - from.num.shift)
}

/// Represents an assigment of template variables
struct TemplateToCSPVal<'a> {
    /// mapping of templates variables into the CSP variables
    mapping: &'a Sub,
    /// Variables of the CSP
    vars: &'a [VarRef],
    /// Values for each variable of the CSP
    vals: &'a [IntCst],
}
impl<'a> PartialAssignment for TemplateToCSPVal<'a> {
    fn val(&self, var: VarRef) -> Option<IntCst> {
        let var = self.mapping.sub_var(var);
        for i in 0..self.vars.len() {
            if var == self.vars[i] {
                return Some(self.vals[i]);
            }
        }
        None
    }
}

fn extract_constraints(
    ch_id: TemplateID,
    mut ch: ChronicleTemplate,
    tr: &Transition,
    pb: &mut Problem,
) -> Option<ChronicleTemplate> {
    let ChronicleLabel::Action(action_name) = ch.label else {
        return None;
    };
    // difference between the transition duration and the action duration
    let duration_delta = delay(ch.chronicle.end, tr.end)? - delay(ch.chronicle.start, tr.start)?;
    // if the action's end time is of the form (start + 10), recover the `10` as an action duration
    let action_fixed_duration = delay(ch.chronicle.start, ch.chronicle.end);

    // gather all variables that appear in the chronicle constraints
    let variables: HashSet<VarRef> = ch
        .chronicle
        .constraints
        .iter()
        .flat_map(|c| c.variables.iter().map(|atom| atom.variable()))
        .collect();

    // variables appearing in the start/end timepoints
    let start_var = ch.chronicle.start.num.var.variable();
    let end_var = ch.chronicle.end.num.var.variable();
    debug_assert!(start_var != end_var || action_fixed_duration.is_some());

    // construct a CSP limited to the constraints that appear in the chronicle
    // this will be used to get a partial grounding of the action
    let mut csp: Model<String> = Model::new();
    // mapping of variables in the original model (appearing in the chronicle) to variables in the CSP
    let mut mapping = Sub::empty();

    // for each variable appearing in the chronicle constraints, create a corresponding varaible in the CSP
    for var in variables {
        if var == start_var || var == end_var {
            // a non-duration constraint applies on the start or end timepoints.
            return None;
        }
        // create a variable with the same bounds.
        // the variable is not optional as we are limiting ourselves to this single chronicle
        let (lb, ub) = pb.context.model.state.bounds(var);
        let new_var = csp.state.new_var(lb, ub);
        // record the correspondance from the original variable to the CSP variable
        mapping.add_untyped(var, new_var).unwrap();
    }
    // the action start is set to 0
    mapping.add_untyped(start_var, VarRef::ZERO).unwrap();
    if end_var != start_var {
        // if there is an end_var, create a corresponding one in the CSP
        let (lb, ub) = pb.context.model.state.bounds(end_var);
        let new_var = csp.state.new_var(lb, ub);
        mapping.add_untyped(end_var, new_var).unwrap();
    }

    // representation of start/end of the chronicle in the new CSP
    let new_start = mapping.fsub(ch.chronicle.start);
    let new_end = mapping.fsub(ch.chronicle.end);

    if ch
        .chronicle
        .constraints
        .iter()
        .all(|c| matches!(c.tpe, ConstraintType::Duration(_) | ConstraintType::Neq))
    {
        // this constraint does not seem to satisfy an interesting "graph-like" pattern
        // TODO: make this analyse more general
        return None;
    }

    // enforce all constraints of the chronicle in the CSP
    for constraint in &ch.chronicle.constraints {
        let c = constraint.substitute(&mapping);
        encode_constraint(&mut csp, &c, Lit::TRUE, new_start, new_end);
    }

    // gather all primary (non-reification) variables appearing in the CSP
    let replacement_vars = mapping.replacement_vars().collect_vec();

    // enumerate all possible values combinations for these variables
    let mut solver = Solver::new(csp);
    let results = solver.enumerate(&replacement_vars).unwrap();

    // the &[IntCst] type represents an assignment to the replacement variables,
    // i.e., one of the solutions to the CSP

    // variables that appear both in the CSP and the state variable
    // these correspond to a subset of the variables of the state variable
    // sufficient to identify its transition graph
    let sv_vars = tr
        .state_var
        .args
        .iter()
        .filter(|v| mapping.contains(v.variable()))
        .map(|s| Atom::Sym(*s))
        .collect_vec();

    // returns true if the given expression is bound by the one parameters of the chronicle
    let bound_by_action_params = |e: Atom| {
        let v = e.variable();
        v == VarRef::ZERO || v == VarRef::ONE || ch.chronicle.name.iter().any(|p| p.variable() == v)
    };
    // only consider as rollable the actions whose transition can be determined directly form the parameters
    // This is necessary to unroll the action in the current implementation but may be relaxed if we were to consider the constraints in the chronicle
    if !bound_by_action_params(tr.pre)
        || !bound_by_action_params(tr.post)
        || sv_vars.iter().any(|v| !bound_by_action_params(*v))
    {
        return None;
    }

    // expression of the action duration
    let dur: IAtom = if let Some(dur) = action_fixed_duration {
        IAtom::from(dur + duration_delta)
    } else {
        ch.chronicle.end.num + duration_delta
    };

    // for each state variable, build a transition graph where the edges are labeled with the transition duration
    let mut graphs: HashMap<SV, Graph> = HashMap::new();
    for a in &results {
        // corresponding assignment for the chronicle's variables
        let ass = TemplateToCSPVal {
            mapping: &mapping,
            vars: &replacement_vars,
            vals: a,
        };

        graphs.entry(ass.evaluate_seq(&sv_vars).unwrap()).or_default().insert(
            ass.evaluate(tr.pre).unwrap(),
            ass.evaluate(tr.post).unwrap(),
            ass.evaluate_int(dur).unwrap(),
        );
    }

    // now we will build a table constraints that contains all possible rolled-up groundings

    // lets first gather all variables that will appear in this constraint:
    //  - variables of the state variable
    //  - variable of the pre- and post-conditions
    //  - new variable for the duration of the rolled up action
    let mut vars = sv_vars.clone();
    vars.push(tr.pre);
    vars.push(tr.post);
    // create a new variable that will be bound to the chronicle duration
    // this is a new variable that should be added to the chronicle parameters
    let dur_var_lbl = Container::Template(ch_id).var(VarType::Reification);

    let duration_var = pb
        .context
        .model
        .new_optional_ivar(0, INT_CST_MAX, ch.chronicle.presence, dur_var_lbl);
    ch.parameters.push(duration_var.into());
    vars.push(duration_var.into());

    // if there is not already an end variable, create a new one
    if action_fixed_duration.is_some() {
        let end_var_lbl = Container::Template(ch_id).var(VarType::ChronicleEnd);
        let end_timepoint = pb.context.model.new_optional_fvar(
            0,
            INT_CST_MAX,
            ch.chronicle.start.denom,
            ch.chronicle.presence,
            end_var_lbl,
        );
        ch.parameters.push(end_timepoint.into());
        ch.chronicle.end = end_timepoint.into();
    }

    // build the table containing all combinations of values for these variables
    let name = format!("duration-rolled-{action_name}");

    // types of the columns
    let types = vars.iter().map(|v| v.tpe()).collect_vec();

    // build the entries in the correct format
    // there is one entry for each shortest path in a transition graph
    let mut entries = Vec::new();
    for (sv, g) in graphs.iter() {
        // for this state variable, add an entry for each shortest path in the graph
        for (src, tgt, cost) in g.apsp() {
            let mut entry = Vec::with_capacity(sv.len() + 3);
            entry.extend_from_slice(sv);
            entry.push(src);
            entry.push(tgt);
            entry.push(Cst::Int(cost));
            entries.push(entry);
        }
    }

    // build the table from the entries
    let mut table = Table::new(name, types);
    for line in &entries {
        table.push(line.as_slice());
    }

    // at this point, we now we will proceed to replacing the action, provide some feedback
    println!(" - {action_name}  ({} entries in table)", entries.len());

    // replace all constraints of the chronicle with:
    // - a table constraint binding variables instantiations to valid rolled up groundings
    // - a duration constraint, based on the introduced duration variable
    ch.chronicle.constraints.clear();
    ch.chronicle.constraints.push(Constraint::table(vars, Arc::new(table)));
    debug_assert_eq!(ch.chronicle.start.denom, ch.chronicle.end.denom);
    let duration = LinearSum::from(FAtom::new(IAtom::from(duration_var), ch.chronicle.start.denom));
    ch.chronicle
        .constraints
        .push(Constraint::duration(Duration::Fixed(duration)));

    // finally reintroduce the condition and effect of the transition
    ch.chronicle.conditions.push(Condition {
        start: ch.chronicle.start,
        end: ch.chronicle.start,
        state_var: tr.state_var.clone(),
        value: tr.pre,
    });
    ch.chronicle.effects.push(Effect {
        transition_start: ch.chronicle.start,
        transition_end: ch.chronicle.end,
        min_mutex_end: vec![],
        state_var: tr.state_var.clone(),
        operation: EffectOp::Assign(tr.post),
    });

    let compilation = RollCompilation {
        chronicle: ch.chronicle.clone(),
        partial_sv: sv_vars,
        src: tr.pre,
        tgt: tr.post,
        graphs,
    };

    // change the label of the chronicle to show that it was rolled up
    ch.label = ChronicleLabel::RolledAction(action_name, Arc::new(compilation));

    Some(ch)
}

/// Identification of the transition graph of a state variable
type SV = Vec<Cst>;
/// Node of the transition graph
type Node = Cst;
/// Cost of a transition, representing the duration of the transition (including the epsilon)
type Cost = IntCst;

#[derive(Default, Clone)]
struct Graph {
    adjacency: HashMap<Node, Vec<(Node, Cost)>>,
}

impl Graph {
    /// inserts a new edge in the graph
    pub fn insert(&mut self, src: Node, tgt: Node, cost: Cost) {
        self.adjacency.entry(src).or_default().push((tgt, cost));
        self.adjacency.entry(tgt).or_default(); // make sure all references nodes have a correspond list
    }

    /// Computes all shortest-path from this node
    pub fn shortest_paths(&self, from: Node) -> impl Iterator<Item = (Node, Cost)> + '_ {
        pathfinding::directed::dijkstra::dijkstra_reach(&from, |n| self.adjacency[n].iter().copied())
            .map(|item| (item.node, item.total_cost))
    }

    /// Returns the set of edges in the shortest path between two nodes
    pub fn shortest_path(&self, from: Node, to: Node) -> Vec<(Node, Node, Cost)> {
        let (parents, res) = pathfinding::directed::dijkstra::dijkstra_partial(
            &from,
            |n| self.adjacency[n].iter().copied(),
            |cur| cur == &to,
        );
        assert_eq!(res, Some(to));
        let path = pathfinding::directed::dijkstra::build_path(&to, &parents);
        assert!(path.len() >= 2);
        let mut edges = Vec::with_capacity(path.len() - 1);
        let mut prev_cost = 0;
        for i in 1..path.len() {
            let cost = parents[&path[i]].1;
            edges.push((path[i - 1], path[i], cost - prev_cost));
            prev_cost = cost;
        }
        edges
    }

    /// Computes the All-Pairs shortest paths (APSP) omitting self-loop paths
    pub fn apsp(&self) -> impl Iterator<Item = (Node, Node, Cost)> + '_ {
        self.adjacency
            .keys()
            .flat_map(|&src| self.shortest_paths(src).map(move |(tgt, cost)| (src, tgt, cost)))
            .filter(|(src, tgt, _)| src != tgt) // for the purpose of rolling (with single transition), a loop to origin is useless (the plan would be non-minimal)
    }
}

pub fn rollup_actions(pb: &mut Problem) {
    println!("Rolling, rolling, rolling... Rawhide!");
    for ch_id in 0..pb.templates.len() {
        let a = &pb.templates[ch_id];

        let Some((tr, ch)) = extract_single_transition(a.clone()) else {
            continue;
        };
        let Some(rolled_chronicle) = extract_constraints(ch_id, ch, &tr, pb) else {
            continue;
        };
        pb.templates[ch_id] = rolled_chronicle;
    }
}

/// The compilation that was used to roll-up an action.
/// Primarily useful to unroll the action in the solution plan
#[derive(Clone)]
pub struct RollCompilation {
    /// Chronicle as it was when compiled
    chronicle: Chronicle,
    /// Parameter of the state variable that identify the transition graph
    partial_sv: Vec<Atom>,
    /// Expression giving the source of the transition
    src: Atom,
    /// Expression giving the target of the transition
    tgt: Atom,
    /// All transition graphs used in the compilation
    graphs: HashMap<SV, Graph>,
}

impl RollCompilation {
    /// Unroll an action into the corresponding sequence of original actions.
    pub fn unroll(&self, action: &ActionInstance) -> Vec<ActionInstance> {
        // first, build the assignment corresponding to the assigment of the instance parameters
        // to the original chronicle parameters
        let mut ass = PartialAssignmentBuilder::new();
        let ch_params = &self.chronicle.name[1..];
        for (i, ch_param) in ch_params.iter().copied().enumerate() {
            ass.add(ch_param, action.params[i]).unwrap();
        }

        // evaluate the elements of the transition

        // identifier of the transition graph
        let sv = ass.evaluate_seq(&self.partial_sv).unwrap();

        // source and target nodes in the graph
        let src = ass.evaluate(self.src).unwrap();
        let tgt = ass.evaluate(self.tgt).unwrap();

        let graph = &self.graphs[&sv];
        let path = graph.shortest_path(src, tgt);

        // function allowing the identification of which parameter defines the value of the corresponding atom
        let param_index = |e: Atom| {
            for (i, p) in ch_params.iter().enumerate() {
                if p.variable() == e.variable() {
                    return i;
                }
            }
            panic!("atom not set by parameters")
        };
        let src_param_index = param_index(self.src);
        let tgt_param_index = param_index(self.tgt);
        assert_eq!(action.params[src_param_index], src);
        assert_eq!(action.params[tgt_param_index], tgt);

        // now rebuild the sequence of actions that was rolled-up
        let mut actions = Vec::with_capacity(path.len());

        let epsilon = Rational::new(1, TIME_SCALE.get());
        let mut next_start = action.start;
        for (src, tgt, dur) in path {
            let mut instance = action.clone();
            instance.params[src_param_index] = src;
            instance.params[tgt_param_index] = tgt;

            instance.start = next_start;
            let dur = Rational::new(dur, TIME_SCALE.get()) - epsilon;
            instance.duration = dur;
            next_start = next_start + dur + epsilon;
            actions.push(instance);
        }

        actions
    }
}
