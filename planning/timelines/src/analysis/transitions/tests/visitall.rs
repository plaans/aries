use aries_solver::core::state::Evaluable;
use aries_solver::lang::ModelView;
use aries_solver::prelude::*;

use crate::boxes::Segment;
use crate::constraints::HasValueAt;
use crate::symbols::ObjectEncoding;
use crate::{
    Effect, EffectOp, FluentParam, FluentsEncoding, IntTerm, Sched, SchedEncoder, Solution, StateVar, Task, TaskId,
    VarCst,
};

/// A `visitall` instance: `num_locs` locations *in a line*, every one of which must be visited, with `num_moves` `move` actions available.
#[derive(Debug)]
pub struct VisitAllLine {
    pub num_locs: usize,
    pub num_moves: usize,
}

impl VisitAllLine {
    fn locs_names(&self) -> Vec<String> {
        (0..self.num_locs).map(|i| format!("loc-x{i}")).collect()
    }
}

#[allow(dead_code)]
pub fn build_and_encode_visitall_line(pb: &VisitAllLine, print: bool) -> SchedEncoder {
    let (sched, _) = build_visitall_line(pb);

    let mut encoder = sched.clone().encoder();
    for c in sched.constraints.iter() {
        c.enforce(&mut encoder);
    }

    if print {
        let effs = encoder.sched.effects.iter().collect::<Vec<_>>();
        let conds = encoder.causal_links.conditions.iter().collect::<Vec<_>>();
        let causal_links = encoder.causal_links.get_links().collect::<Vec<_>>();

        println!("Effects:");
        for (eid, e) in effs.iter().enumerate() {
            println!("  {eid}: {e:?}");
        }
        println!("Conditions:");
        for (cid, c) in conds.iter().enumerate() {
            println!("  {cid}: {c:?}");
        }
        println!("Causal Links:");
        for cl in causal_links.iter() {
            println!("  {cl:?}");
        }
    }

    encoder
}

pub fn build_visitall_line(pb: &VisitAllLine) -> (Sched, Vec<Move>) {
    let names = pb.locs_names();

    let objects = ObjectEncoding::build(
        "object".into(),
        |t| match t.as_str() {
            "object" => vec!["loc".into()],
            _ => vec![],
        },
        {
            let names = names.clone();
            move |t| match t.as_str() {
                "loc" => names.clone(),
                _ => vec![],
            }
        },
    );

    let locs = objects.domain_of_type("loc").unwrap();
    let loc_range = Segment::new(locs.first, locs.last);
    let bool_range = Segment::new(0, 1);

    let mut fluents = FluentsEncoding::empty();
    fluents.add(
        "connected".into(),
        &[FluentParam { range: loc_range }, FluentParam { range: loc_range }],
        FluentParam { range: bool_range },
    );
    fluents.add(
        "at-robot".into(),
        &[FluentParam { range: loc_range }],
        FluentParam { range: bool_range },
    );
    fluents.add(
        "visited".into(),
        &[FluentParam { range: loc_range }],
        FluentParam { range: bool_range },
    );

    let mut model = Sched::new(1, objects, fluents);

    let loc: Vec<IntCst> = names.iter().map(|n| model.objects.object_id(n).unwrap()).collect();

    // (:init …): robot at loc-x0, which is already visited, and a bidirectional chain
    init_bool(&mut model, "at-robot", &[loc[0]], true);
    init_bool(&mut model, "visited", &[loc[0]], true);
    for w in loc.windows(2) {
        init_bool(&mut model, "connected", &[w[0], w[1]], true);
        init_bool(&mut model, "connected", &[w[1], w[0]], true);
    }

    // (:goal (and (visited loc-x0) … ))
    for &l in &loc {
        model.add_constraint(HasValueAt {
            state_var: state_var("visited", vec![l.into()]),
            value: IntTerm::TRUE,
            timepoint: model.horizon,
            prez: Lit::TRUE,
            source: None,
        });
    }

    let moves = (0..pb.num_moves).map(|_| add_move(&mut model, &loc)).collect();
    (model, moves)
}

/// (:action move :parameters (?curpos ?nextpos - loc))
fn add_move(model: &mut Sched, loc: &[IntCst]) -> Move {
    let presence = model.new_bool_var();
    let start: VarCst = model.new_opt_timepoint(presence);
    let end: VarCst = start + 1;

    let (first, last) = (*loc.first().unwrap(), *loc.last().unwrap());
    let curpos = model.new_optional_var(first, last, presence);
    let nextpos = model.new_optional_var(first, last, presence);

    let task_id = model.add_task(Task {
        name: "move".into(),
        start,
        end,
        presence,
        args: vec![curpos.into(), nextpos.into()],
    });

    // :precondition (and (at-robot ?curpos) (connected ?curpos ?nextpos))
    for sv in [
        state_var("at-robot", vec![curpos.into()]),
        state_var("connected", vec![curpos.into(), nextpos.into()]),
    ] {
        model.add_constraint(HasValueAt {
            state_var: sv,
            value: IntTerm::TRUE,
            timepoint: start,
            prez: presence,
            source: Some(task_id),
        });
    }

    // :effect (and (at-robot ?nextpos) (not (at-robot ?curpos)) (visited ?nextpos))
    for (sv, value) in [
        (state_var("at-robot", vec![nextpos.into()]), IntTerm::TRUE),
        (state_var("at-robot", vec![curpos.into()]), IntTerm::ZERO),
        (state_var("visited", vec![nextpos.into()]), IntTerm::TRUE),
    ] {
        let mutex_end = model.new_opt_timepoint(presence);
        model.add_effect(Effect {
            transition_start: start,
            transition_end: end,
            mutex_end,
            state_var: sv,
            operation: EffectOp::Assign(value),
            prez: presence,
            source: Some(task_id),
        });
    }

    Move {
        presence,
        start,
        curpos,
        nextpos,
        _task_id: task_id,
    }
}

#[derive(Debug)]
pub struct Move {
    presence: Lit,
    start: VarCst,
    curpos: Var,
    nextpos: Var,
    _task_id: TaskId,
}

impl Evaluable for Move {
    type Value = (IntCst, IntCst, IntCst);

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        if !solution.entails(self.presence) {
            return None;
        }
        Some((
            solution.eval(self.start).unwrap(),
            solution.eval(self.curpos).unwrap(),
            solution.eval(self.nextpos).unwrap(),
        ))
    }
}

fn state_var(fluent: &str, args: Vec<IntTerm>) -> StateVar {
    StateVar {
        fluent: fluent.into(),
        args,
    }
}

fn init_bool(model: &mut Sched, fluent: &str, args: &[IntCst], value: bool) {
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: model.origin,
        transition_end: model.origin,
        mutex_end,
        state_var: state_var(fluent, args.iter().map(|&a| a.into()).collect()),
        operation: EffectOp::Assign(if value { IntTerm::TRUE } else { IntTerm::ZERO }),
        prez: Lit::TRUE,
        source: None,
    });
}

#[cfg(test)]
mod test {
    use super::{VisitAllLine, build_and_encode_visitall_line};

    #[test]
    fn build_simple_visitall_line() {
        build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 5,
                num_moves: 4,
            },
            true,
        );
    }
}
