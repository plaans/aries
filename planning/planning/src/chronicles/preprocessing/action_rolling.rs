use crate::chronicles::analysis::TemplateID;
use crate::chronicles::constraints::{encode_constraint, Constraint, ConstraintType, Duration, Table};
use crate::chronicles::{
    ChronicleLabel, ChronicleTemplate, Condition, Container, DiscreteValue, Effect, EffectOp, Problem, StateVar, Sub,
    Substitute, Substitution, Time, VarType,
};
use aries::core::state::Term;
use aries::core::{IntCst, Lit, VarRef, INT_CST_MAX};
use aries::model::lang::linear::LinearSum;
use aries::model::lang::{Atom, FAtom, IAtom, IVar, SVar, Type};
use aries::model::symbols::{SymId, TypedSym};
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
    let is_constant = |atom: Atom| DiscreteValue::try_from(atom).is_ok();
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

    // returns the index of an original variable in the replacement-vars array
    let index = |var: VarRef| {
        let sub_var = mapping.sub_var(var);
        replacement_vars
            .iter()
            .position(|e| *e == sub_var)
            .expect("no variable for pre")
    };

    // the &[IntCst] type represents an assignment to the replacement variables,
    // i.e., one of the solutions to the CSP

    // returns the value of an original variable in this assignements
    let val = |var: VarRef, ass: &[IntCst]| ass[index(var)];
    // variables that appear both in the CSP and the state variable
    let sv_vars = tr
        .state_var
        .args
        .iter()
        .map(|arg| arg.variable())
        .filter(|v| mapping.contains(*v))
        .collect_vec();
    let sv_indices = sv_vars.iter().copied().map(index).collect_vec();
    // returns the partial assignment to the sv vars
    let sv = |ass: &[IntCst]| sv_indices.iter().map(|i| ass[*i]).collect_vec();
    // returns the value of the transition precondition  in the assignment
    let src = |ass: &[IntCst]| val(tr.pre.variable(), ass);
    // returns the value of the transition post-condition in the assignment
    let tgt = |ass: &[IntCst]| val(tr.post.variable(), ass);
    // duration of the transition for this assignment
    let dur = |ass: &[IntCst]| {
        if let Some(dur) = action_fixed_duration {
            dur + duration_delta
        } else {
            val(Atom::Fixed(ch.chronicle.end).variable(), ass) + duration_delta
        }
    };

    // for each state variable, build a transition graph where the edges are labeled with the transition duration
    let mut graphs: HashMap<SV, Graph> = HashMap::new();
    for a in &results {
        graphs.entry(sv(a)).or_default().insert(src(a), tgt(a), dur(a));
    }

    // now we will build a table constraints that contains all possible rolled-up groundings

    // lets first gather all variables that will appear in this constraint:
    //  - variables of the state variable
    //  - variable of the pre- and post-conditions
    //  - new variable for the duration of the rolled up action
    let mut vars = sv_vars.clone();
    vars.push(tr.pre.variable());
    vars.push(tr.post.variable());
    // create a new variable that will be bound to the chronicle duration
    // this is a new variable that should be added to the chronicle parameters
    let dur_var_lbl = Container::Template(ch_id).var(VarType::Reification);

    let duration_var = pb
        .context
        .model
        .new_optional_ivar(0, INT_CST_MAX, ch.chronicle.presence, dur_var_lbl);
    ch.parameters.push(duration_var.into());
    vars.push(duration_var.variable());

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

    // build the table containing all combinations of values for these varaibles
    let name = format!("duration-rolled-{action_name}");
    // types of the variables (useful because the table constraint has a typed representation that we need to recover)
    let types = vars.iter().map(|v| pb.context.model.shape.types[*v]).collect_vec();

    // returns the DiscreteValue for the constant and the given type
    let dv = |tpe: Type, value: IntCst| match tpe {
        Type::Sym(t) => {
            assert!((0..INT_CST_MAX).contains(&value));
            DiscreteValue::Sym(TypedSym::new(SymId::from_u32(value as u32), t))
        }
        Type::Int { lb, ub } => {
            assert!(value >= lb && value <= ub);
            DiscreteValue::Int(value)
        }
        Type::Fixed(_) => todo!(),
        Type::Bool => todo!(),
    };
    // returns the Atom for the variable and the given type
    let atom = |tpe: Type, var: VarRef| -> Atom {
        match tpe {
            Type::Sym(t) => SVar::new(var, t).into(),
            Type::Int { .. } => IVar::new(var).into(),
            Type::Fixed(_) => todo!(),
            Type::Bool => todo!(),
        }
    };
    // variables (as Atom) that will be bound by the table constraint
    let vars = vars
        .iter()
        .enumerate()
        .map(|(i, var)| atom(types[i], *var))
        .collect_vec();

    // build the entries in the correct format
    // there is one entry for each shortest path in a transition graph
    let mut entries = Vec::new();
    for (sv, g) in graphs.iter() {
        let sv = sv.iter().enumerate().map(|(i, val)| dv(types[i], *val)).collect_vec();

        // for this state variable, add an entry for each shortest path in the graph
        for (src, tgt, cost) in g.apsp() {
            let mut entry = Vec::with_capacity(sv.len() + 3);
            entry.extend_from_slice(&sv);
            entry.push(dv(types[sv.len()], src));
            entry.push(dv(types[sv.len() + 1], tgt));
            entry.push(dv(types[sv.len() + 2], cost));
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

    // change the label of the chronicle to show that it was rolled up
    ch.label = ChronicleLabel::RolledAction(action_name);

    Some(ch)
}

type SV = Vec<IntCst>;
type Node = IntCst;
type Cost = IntCst;

#[derive(Default)]
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
        pathfinding::directed::dijkstra::dijkstra_reach(&from, |n, _c| self.adjacency[&n].iter().copied())
            .map(|item| (item.node, item.total_cost))
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
