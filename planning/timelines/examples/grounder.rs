use std::collections::HashMap;

use aries_solver::lang::ModelView;
use aries_solver::prelude::*;
use aries_timelines::boxes::Segment;
use aries_timelines::constraints::HasValueAt;
use aries_timelines::ext::ground::SourceGrounding;
use aries_timelines::symbols::ObjectEncoding;
use aries_timelines::*;
use idmap::intid::IntegerId;
use itertools::Itertools;

fn main() {
    println!("=== Nonlifted navigation (boolean predicates) ===");
    println!();
    run("nonlifted", &make_nonlifted_nav());

    println!();
    println!("=== Lifted navigation (state variables) ===");
    println!();
    run("lifted", &make_lifted_nav());
}

fn run(label: &str, sched: &Sched) {
    let grounder = sched.simple_datalog_grounder(true);
    if let Some(view) = grounder.get_view() {
        println!("Datalog program ({label}):");
        view.print();
    }
    println!();

    let datalog = grounder.run();
    println!("Datalog groundings ({label}):");
    print_groundings(&datalog, sched);

    println!();

    let naive = naive_groundings(sched);
    println!("Naive groundings ({label}):");
    print_groundings(&naive, sched);

    println!("---");
}

fn shared_objects() -> (ObjectEncoding, IntCst, IntCst, IntCst) {
    let objects = ObjectEncoding::build(
        "object".into(),
        |t| match t.as_str() {
            "object" => vec!["room".into()],
            _ => vec![],
        },
        |t| match t.as_str() {
            "room" => vec!["a".into(), "b".into(), "c".into()],
            _ => vec![],
        },
    );
    let a = objects.object_id("a").unwrap();
    let b = objects.object_id("b").unwrap();
    let c = objects.object_id("c").unwrap();
    (objects, a, b, c)
}

fn make_nonlifted_nav() -> Sched {
    let (objects, a, b, c) = shared_objects();

    let mut fluents = FluentsEncoding::empty();
    let any = Segment::all();
    let bool_range = Segment::new(0, 1);

    fluents.add(
        "at".into(),
        &[FluentParam {
            range: any,
            tpe: "room".into(),
        }],
        FluentParam {
            range: bool_range,
            tpe: "bool".into(),
        },
    );
    fluents.add(
        "connected".into(),
        &[
            FluentParam {
                range: any,
                tpe: "room".into(),
            },
            FluentParam {
                range: any,
                tpe: "room".into(),
            },
        ],
        FluentParam {
            range: bool_range,
            tpe: "bool".into(),
        },
    );

    let mut sched = Sched::new(1, objects, fluents);

    init_bool(&mut sched, "at", &[a], true);
    init_bool(&mut sched, "connected", &[a, b], true);
    init_bool(&mut sched, "connected", &[b, c], true);

    sched.add_constraint(HasValueAt {
        state_var: StateVar {
            fluent: "at".into(),
            args: vec![c.into()],
        },
        value: IntTerm::TRUE,
        timepoint: sched.horizon,
        prez: Lit::TRUE,
        source: None,
    });

    add_go_nonlifted(&mut sched);
    sched
}

fn make_lifted_nav() -> Sched {
    let (objects, a, b, c) = shared_objects();

    let mut fluents = FluentsEncoding::empty();
    let any = Segment::all();
    let bool_range = Segment::new(0, 1);

    fluents.add(
        "robot_at".into(),
        &[],
        FluentParam {
            range: Segment::new(a, c),
            tpe: "room".into(),
        },
    );
    fluents.add(
        "connected".into(),
        &[
            FluentParam {
                range: any,
                tpe: "room".into(),
            },
            FluentParam {
                range: any,
                tpe: "room".into(),
            },
        ],
        FluentParam {
            range: bool_range,
            tpe: "bool".into(),
        },
    );

    let mut sched = Sched::new(1, objects, fluents);

    init_val(&mut sched, "robot_at", &[], a);
    init_val(&mut sched, "connected", &[a, b], 1);
    init_val(&mut sched, "connected", &[b, c], 1);

    sched.add_constraint(HasValueAt {
        state_var: StateVar {
            fluent: "robot_at".into(),
            args: vec![],
        },
        value: IntTerm::from(c),
        timepoint: sched.horizon,
        prez: Lit::TRUE,
        source: None,
    });

    add_go_lifted(&mut sched);
    sched
}

fn add_go_nonlifted(sched: &mut Sched) -> TaskId {
    let presence = sched.new_bool_var();
    let start = sched.new_opt_timepoint(presence);
    let end = start + 1;

    let room = sched.objects.domain_of_type("room").unwrap();
    let from = sched.new_optional_var(room.first, room.last, presence);
    let to = sched.new_optional_var(room.first, room.last, presence);

    let task_id = sched.add_task(Task {
        name: "go".into(),
        start,
        end,
        presence,
        args: vec![(from.into(), "room".into()), (to.into(), "room".into())],
    });

    sched.add_constraint(HasValueAt {
        state_var: StateVar {
            fluent: "at".into(),
            args: vec![from.into()],
        },
        value: IntTerm::TRUE,
        timepoint: start,
        prez: presence,
        source: Some(task_id),
    });
    sched.add_constraint(HasValueAt {
        state_var: StateVar {
            fluent: "connected".into(),
            args: vec![from.into(), to.into()],
        },
        value: IntTerm::TRUE,
        timepoint: start,
        prez: presence,
        source: Some(task_id),
    });

    let me = sched.new_opt_timepoint(presence);
    sched.add_effect(Effect {
        transition_start: start,
        transition_end: end,
        mutex_end: me,
        state_var: StateVar {
            fluent: "at".into(),
            args: vec![to.into()],
        },
        operation: EffectOp::Assign(IntTerm::TRUE),
        prez: presence,
        source: Some(task_id),
    });

    let me = sched.new_opt_timepoint(presence);
    sched.add_effect(Effect {
        transition_start: start,
        transition_end: end,
        mutex_end: me,
        state_var: StateVar {
            fluent: "at".into(),
            args: vec![from.into()],
        },
        operation: EffectOp::Assign(IntTerm::ZERO),
        prez: presence,
        source: Some(task_id),
    });

    task_id
}

fn add_go_lifted(sched: &mut Sched) -> TaskId {
    let presence = sched.new_bool_var();
    let start = sched.new_opt_timepoint(presence);
    let end = start + 1;

    let room = sched.objects.domain_of_type("room").unwrap();
    let from = sched.new_optional_var(room.first, room.last, presence);
    let to = sched.new_optional_var(room.first, room.last, presence);

    let task_id = sched.add_task(Task {
        name: "go".into(),
        start,
        end,
        presence,
        args: vec![(from.into(), "room".into()), (to.into(), "room".into())],
    });

    sched.add_constraint(HasValueAt {
        state_var: StateVar {
            fluent: "robot_at".into(),
            args: vec![],
        },
        value: from.into(),
        timepoint: start,
        prez: presence,
        source: Some(task_id),
    });
    sched.add_constraint(HasValueAt {
        state_var: StateVar {
            fluent: "connected".into(),
            args: vec![from.into(), to.into()],
        },
        value: IntTerm::TRUE,
        timepoint: start,
        prez: presence,
        source: Some(task_id),
    });

    let me = sched.new_opt_timepoint(presence);
    sched.add_effect(Effect {
        transition_start: start,
        transition_end: end,
        mutex_end: me,
        state_var: StateVar {
            fluent: "robot_at".into(),
            args: vec![],
        },
        operation: EffectOp::Assign(to.into()),
        prez: presence,
        source: Some(task_id),
    });

    task_id
}

fn init_bool(sched: &mut Sched, fluent: &str, args: &[IntCst], value: bool) {
    let mutex_end = sched.new_timepoint();
    sched.add_effect(Effect {
        transition_start: sched.origin,
        transition_end: sched.origin,
        mutex_end,
        state_var: StateVar {
            fluent: fluent.into(),
            args: args.iter().map(|&a| a.into()).collect(),
        },
        operation: EffectOp::Assign(if value { IntTerm::TRUE } else { IntTerm::ZERO }),
        prez: Lit::TRUE,
        source: None,
    });
}

fn init_val(sched: &mut Sched, fluent: &str, args: &[IntCst], value: IntCst) {
    let mutex_end = sched.new_timepoint();
    sched.add_effect(Effect {
        transition_start: sched.origin,
        transition_end: sched.origin,
        mutex_end,
        state_var: StateVar {
            fluent: fluent.into(),
            args: args.iter().map(|&a| a.into()).collect(),
        },
        operation: EffectOp::Assign(IntTerm::from(value)),
        prez: Lit::TRUE,
        source: None,
    });
}

fn print_groundings(groundings: &HashMap<Option<TaskId>, Vec<SourceGrounding>>, sched: &Sched) {
    let decoder = sched.objects.decoder();
    for (source, gs) in groundings {
        match source {
            None => println!("  [global] {} grounding(s)", gs.len()),
            Some(task_id) => {
                let task = &sched.tasks[*task_id];
                println!("  [{}] {} grounding(s)", task.name, gs.len());
                for g in gs {
                    let decoded: Vec<String> = g
                        .inner()
                        .iter()
                        .map(|v| decoder.decode(*v).cloned().unwrap_or_else(|| format!("{}", v)))
                        .collect();
                    println!("    ({})", decoded.iter().format(", "));
                }
            }
        }
    }
}

fn naive_groundings(sched: &Sched) -> HashMap<Option<TaskId>, Vec<SourceGrounding>> {
    let mut result = HashMap::new();

    for (idx, task) in sched.tasks.iter().enumerate() {
        let task_id = TaskId::from_int(idx as u32);

        let domains: Vec<_> = task
            .args
            .iter()
            .filter_map(|(term, type_name)| {
                if term.is_cst() {
                    return None;
                }
                let range = sched.objects.domain_of_type(type_name)?;
                Some(range.first..=range.last)
            })
            .collect();

        let gs: Vec<SourceGrounding> = domains
            .iter()
            .cloned()
            .multi_cartesian_product()
            .map(SourceGrounding::from)
            .collect();
        result.insert(Some(task_id), gs);
    }

    result.insert(None, vec![SourceGrounding::from(vec![])]);
    result
}
