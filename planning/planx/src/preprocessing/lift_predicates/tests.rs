use std::path::Path;

use crate::pddl::{build_model, input::Input, parser};
use crate::{EffectOp, Model, Res, Sym, TimeRef, Type};

fn lift(domain: Input, problem: Input) -> Res<Model> {
    let domain = parser::parse_pddl_domain(domain)?;
    let problem = parser::parse_pddl_problem(problem)?;
    let mut model = build_model(&domain, &problem)?;
    super::lift_predicates_to_state_functions(&mut model)?;
    println!("{model}");
    Ok(model)
}

fn lift_files(dir: &str, domain: &str, problem: &str) -> Res<Model> {
    let dir = Path::new("../problems").join(dir);
    lift(
        Input::from_file(&dir.join(domain))?,
        Input::from_file(&dir.join(problem))?,
    )
}

fn lift_str(domain: &str, problem: &str) -> Model {
    lift(Input::from_string(domain), Input::from_string(problem)).unwrap()
}

/// Names of the state functions resulting from the lifting, sorted.
fn lifted(model: &Model) -> Vec<String> {
    let mut names: Vec<String> = (model.env.fluents.iter())
        .filter(|f| matches!(f.return_type, Type::User(_)))
        .map(|f| f.name().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn gripper() -> Res<()> {
    let model = lift_files(
        "pddl/ipc/1998-gripper-round-1-strips",
        "domain.pddl",
        "instance.1.pb.pddl",
    )?;
    // values of `at` (rooms) and `carry` (grippers) are distinguished thanks to the static `room`/`gripper` predicates
    assert_eq!(lifted(&model), ["at-robby", "at:carry"]);
    Ok(())
}

#[test]
fn satellite_time() -> Res<()> {
    let model = lift_files(
        "upf/ipc2002-satellite-time-simple-automatic",
        "domain.pddl",
        "problem.pddl",
    )?;
    assert_eq!(
        lifted(&model),
        [
            "calibration_target",
            "on_board",
            "pointing",
            "power_avail:power_on",
            "supports"
        ]
    );

    // pointing is deleted at start and added at end: erased at start, assigned at end
    let turn_to = model.actions.get_action(&Sym::from("turn_to")).unwrap();
    assert_eq!(turn_to.effects.len(), 2);
    for eff in &turn_to.effects {
        let eff = &eff.effect_expression;
        assert_eq!(model.env.fluents.get(eff.state_variable.fluent).name(), "pointing");
        assert_eq!(eff.state_variable.arguments.len(), 1);
        match eff.transition_start.reference {
            TimeRef::ActionStart => assert!(matches!(eff.operation, EffectOp::Erase)),
            TimeRef::ActionEnd => assert!(matches!(eff.operation, EffectOp::Assign(_))),
            _ => panic!(),
        }
    }
    Ok(())
}

#[test]
fn psr() -> Res<()> {
    let model = lift_files("upf/ipc2004-psr-small-strips", "domain.pddl", "problem.pddl")?;
    assert_eq!(
        lifted(&model),
        [
            "do_close_sd1_condeffs:do_wait_cb1_condeffs:do_normal",
            "not_closed_cb1:closed_cb1",
            "not_closed_sd1:closed_sd1",
            "not_closed_sd2:closed_sd2",
            "updated_cb1:not_updated_cb1"
        ]
    );
    Ok(())
}

#[test]
fn rovers() -> Res<()> {
    let model = lift_files("upf/ipc2002-rovers-strips-automatic", "domain.pddl", "problem.pddl")?;
    assert_eq!(lifted(&model).len(), 15);
    assert!(lifted(&model).contains(&"empty:full".to_string()));
    // a sample is either at its waypoint or analysed by a rover
    assert!(lifted(&model).contains(&"have_rock_analysis:at_rock_sample".to_string()));
    assert!(lifted(&model).contains(&"at_".to_string()));
    Ok(())
}

#[test]
fn rovers_time() -> Res<()> {
    // `empty:full` must NOT be lifted: two overlapping `drop` on the same store may lead to both being true
    let model = lift_files(
        "upf/ipc2002-rovers-time-simple-automatic",
        "domain.pddl",
        "problem.pddl",
    )?;
    assert!(lifted(&model).iter().all(|g| !g.contains("full")));
    Ok(())
}

#[test]
#[ignore] // problems in `planning/ext/` are not always guaranteed to be present
fn cushing() -> Res<()> {
    // `target1` and `target3` are both true in the goal: they must not be merged
    let model = lift_files(
        "../ext/pddl/ipc-2018/domains/cushing-temporal-satisficing",
        "domain.pddl",
        "instances/instance-1.pddl",
    )?;
    assert!(lifted(&model).iter().all(|g| !g.contains("target")));
    Ok(())
}

// Synthetic domains exercising each condition

const SWITCH_PB: &str =
    "(define (problem p) (:domain d) (:objects d1 d2 - dev) (:init (off d1) (on d2)) (:goal (on d1)))";

fn switch_domain(on_pre: &str, off_pre: &str, extra: &str) -> String {
    format!(
        "(define (domain d) (:requirements :typing) (:types dev)
           (:predicates (on ?d - dev) (off ?d - dev))
           (:action turn-on :parameters (?d - dev) :precondition (and {on_pre}) :effect (and (on ?d) (not (off ?d))))
           (:action turn-off :parameters (?d - dev) :precondition (and {off_pre}) :effect (and (off ?d) (not (on ?d))))
           {extra})"
    )
}

#[test]
fn balanced_actions() {
    let model = lift_str(&switch_domain("(off ?d)", "(on ?d)", ""), SWITCH_PB);
    assert_eq!(lifted(&model), ["on:off"]);
    // the delete is subsumed by the assignment
    let turn_on = model.actions.get_action(&Sym::from("turn-on")).unwrap();
    assert_eq!(turn_on.effects.len(), 1);
    assert!(matches!(
        turn_on.effects[0].effect_expression.operation,
        EffectOp::Assign(_)
    ));
}

#[test]
fn unrequired_delete() {
    // `?from` is not required: `?o` may be elsewhere, and would end up at two locations
    let domain = move_domain("").replace("(at ?o ?from) :effect", "(and) :effect");
    let model = lift_str(&domain, MOVE_PB);
    assert!(lifted(&model).is_empty());
}

#[test]
fn negative_occurrence() {
    let negated_goal = SWITCH_PB.replace("(:goal (on d1))", "(:goal (not (on d1)))");
    let model = lift_str(&switch_domain("(off ?d)", "(on ?d)", ""), &negated_goal);
    assert!(lifted(&model).is_empty());
}

#[test]
fn initial_state_violation() {
    let both = SWITCH_PB.replace("(off d1)", "(off d1) (on d1)");
    let model = lift_str(&switch_domain("(off ?d)", "(on ?d)", ""), &both);
    assert!(lifted(&model).is_empty());
}

#[test]
fn unbalanced_extra_action() {
    // an extra action adds `on` without deleting anything
    let extra = "(:action force :parameters (?d - dev) :precondition (and) :effect (on ?d))";
    let model = lift_str(&switch_domain("(off ?d)", "(on ?d)", extra), SWITCH_PB);
    assert!(lifted(&model).is_empty());
}

#[test]
fn covering_deletes() {
    // `reset` does not require anything but deletes all other atoms of the key: precise (P3')
    let extra = "(:action reset :parameters (?d - dev) :precondition (and) :effect (and (off ?d) (not (on ?d))))";
    let model = lift_str(&switch_domain("(off ?d)", "(on ?d)", extra), SWITCH_PB);
    assert_eq!(lifted(&model), ["on:off"]);
}

const MOVE_PB: &str = "(define (problem p) (:domain d) (:objects a b - obj l1 l2 - loc)
    (:init (at a l1) (at b l1)) (:goal (and (at a l2) (at b l2))))";

fn move_domain(extra_pre: &str) -> String {
    format!(
        "(define (domain d) (:requirements :typing :equality) (:types obj loc)
           (:predicates (at ?o - obj ?l - loc))
           (:action move :parameters (?o - obj ?from ?to - loc)
             :precondition (at ?o ?from) :effect (and (not (at ?o ?from)) (at ?o ?to)))
           (:action move2 :parameters (?o1 ?o2 - obj ?from ?to - loc)
             :precondition (and (at ?o1 ?from) (at ?o2 ?from) {extra_pre})
             :effect (and (not (at ?o1 ?from)) (at ?o1 ?to) (not (at ?o2 ?from)) (at ?o2 ?to))))"
    )
}

#[test]
fn unrequired_delete_covering() {
    // turn-off does not require `on` but deletes all other atoms of the key (P3')
    let model = lift_str(&switch_domain("(off ?d)", "", ""), SWITCH_PB);
    assert_eq!(lifted(&model), ["on:off"]);
}

#[test]
fn keys_may_unify() {
    // in move2, ?o1 and ?o2 may be the same object, leading to two assignments of the same state variable
    let model = lift_str(&move_domain(""), MOVE_PB);
    assert!(lifted(&model).is_empty());
}

#[test]
fn keys_declared_distinct() {
    let model = lift_str(&move_domain("(not (= ?o1 ?o2))"), MOVE_PB);
    assert_eq!(lifted(&model), ["at"]);
    let move2 = model.actions.get_action(&Sym::from("move2")).unwrap();
    assert_eq!(move2.effects.len(), 2);
}

fn temporal_domain(drop_condition: &str) -> String {
    temporal_domain_with_duration(drop_condition, "2")
}

fn temporal_domain_with_duration(drop_condition: &str, drop_duration: &str) -> String {
    format!(
        "(define (domain d) (:requirements :typing :durative-actions :fluents) (:types store)
           (:predicates (empty ?s - store) (full ?s - store))
           (:functions (speed ?s - store))
           (:durative-action fill :parameters (?s - store) :duration (= ?duration 2)
             :condition (at start (empty ?s))
             :effect (and (at start (not (empty ?s))) (at end (full ?s))))
           (:durative-action drop :parameters (?s - store) :duration (= ?duration {drop_duration})
             :condition {drop_condition}
             :effect (and (at end (not (full ?s))) (at end (empty ?s)))))"
    )
}

const STORE_PB: &str = "(define (problem p) (:domain d) (:objects s - store) (:init (empty s)) (:goal (full s)))";

#[test]
fn temporal_required_at_start_only() {
    // overlapping drops may result in both empty and full holding (see rovers-time)
    let model = lift_str(&temporal_domain("(at start (full ?s))"), STORE_PB);
    assert!(lifted(&model).is_empty());
}

#[test]
fn temporal_required_over_all() {
    // `full` holds until the end of drop, where it is deleted: precise
    let model = lift_str(&temporal_domain("(over all (full ?s))"), STORE_PB);
    assert_eq!(lifted(&model), ["empty:full"]);
    let fill = model.actions.get_action(&Sym::from("fill")).unwrap();
    assert!(matches!(fill.effects[0].effect_expression.operation, EffectOp::Erase));
}

#[test]
fn temporal_required_over_all_with_computed_duration() {
    // the delete at end is required by the `over all` condition only if the duration is provably positive
    let domain = temporal_domain_with_duration("(over all (full ?s))", "(/ 1 (speed ?s))");
    let with_speed = |speed: &str| STORE_PB.replace("(empty s)", &format!("(empty s) (= (speed s) {speed})"));
    assert_eq!(lifted(&lift_str(&domain, &with_speed("4"))), ["empty:full"]);
    assert!(lifted(&lift_str(&domain, &with_speed("-4"))).is_empty());
}

fn power_domain(switch_on_pre: &str) -> String {
    format!(
        "(define (domain d) (:requirements :typing) (:types instrument satellite)
           (:predicates (on_board ?i - instrument ?s - satellite) (power_avail ?s - satellite) (power_on ?i - instrument))
           (:action switch_on :parameters (?i - instrument ?s - satellite)
             :precondition (and {switch_on_pre} (power_avail ?s))
             :effect (and (power_on ?i) (not (power_avail ?s))))
           (:action switch_off :parameters (?i - instrument ?s - satellite)
             :precondition (and (on_board ?i ?s) (power_on ?i))
             :effect (and (not (power_on ?i)) (power_avail ?s))))"
    )
}

fn power_pb(on_board: &str) -> String {
    format!(
        "(define (problem p) (:domain d) (:objects i1 i2 - instrument s1 s2 - satellite)
           (:init {on_board} (power_avail s1) (power_avail s2)) (:goal (and (power_on i1) (power_on i2))))"
    )
}

#[test]
fn functional_key() {
    // each instrument is on a single satellite: the key of `power_on(?i)` is the satellite of `?i`
    let model = lift_str(
        &power_domain("(on_board ?i ?s)"),
        &power_pb("(on_board i1 s1) (on_board i2 s1)"),
    );
    assert!(lifted(&model).contains(&"power_avail:power_on".to_string()));
}

#[test]
fn functional_key_not_a_function() {
    // i1 is on two satellites: switching it on from s1 and from s2 makes it on twice
    let model = lift_str(
        &power_domain("(on_board ?i ?s)"),
        &power_pb("(on_board i1 s1) (on_board i1 s2) (on_board i2 s1)"),
    );
    assert!(!lifted(&model).contains(&"power_avail:power_on".to_string()));
}

#[test]
fn functional_key_unbound() {
    // switch_on does not require the instrument to be on the satellite whose power it uses
    let model = lift_str(&power_domain(""), &power_pb("(on_board i1 s1) (on_board i2 s1)"));
    assert!(!lifted(&model).contains(&"power_avail:power_on".to_string()));
}

#[test]
fn no_rekeying() {
    // `{robot-at(?r, ?t), clear(?t)}` keyed by the tile is valid, but `robot-at` can be lifted alone,
    // keyed by the robot, which is kept
    let domain = "(define (domain d) (:requirements :typing) (:types robot tile)
        (:predicates (robot-at ?r - robot ?t - tile) (clear ?t - tile))
        (:action move :parameters (?r - robot ?from ?to - tile)
          :precondition (and (robot-at ?r ?from) (clear ?to))
          :effect (and (robot-at ?r ?to) (not (robot-at ?r ?from)) (clear ?from) (not (clear ?to)))))";
    let pb = "(define (problem p) (:domain d) (:objects r - robot t1 t2 t3 - tile)
        (:init (robot-at r t1) (clear t2) (clear t3)) (:goal (robot-at r t2)))";
    let model = lift_str(domain, pb);
    assert_eq!(lifted(&model), ["robot-at"]);
    let robot_at = model.env.fluents.iter().find(|f| f.name() == "robot-at").unwrap();
    assert_eq!(robot_at.parameters[0].tpe().to_string(), "robot");
}
