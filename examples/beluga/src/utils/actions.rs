use std::fmt;

use aries_solver::core::state::Evaluable;
use aries_solver::lang::{CoreExpr, ModelView};
use aries_solver::prelude::*;
use aries_timelines::{constraints::HasValueAt, *};

use super::instance::JigHolder;
use super::states::*;
use super::*;

#[derive(Debug)]
pub enum ActionType {
    LoadBeluga {
        j: VarCst,
        b: VarCst,
        t: VarCst,
    },
    UnloadBeluga {
        j: VarCst,
        b: VarCst,
        t: VarCst,
    },
    GetFromHangar {
        j: VarCst,
        h: VarCst,
        t: VarCst,
    },
    DeliverToHangar {
        j: VarCst,
        h: VarCst,
        t: VarCst,
        pl: VarCst,
    },
    PutDownRack {
        j: VarCst,
        t: VarCst,
        r: VarCst,
        side: VarCst,
    },
    PickUpRack {
        j: VarCst,
        t: VarCst,
        r: VarCst,
        side: VarCst,
    },
    SwitchToNextBeluga,
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionType::LoadBeluga { .. } => write!(f, "LoadBeluga"),
            ActionType::UnloadBeluga { .. } => write!(f, "UnloadBeluga"),
            ActionType::GetFromHangar { .. } => write!(f, "GetFromHangar"),
            ActionType::DeliverToHangar { .. } => write!(f, "DeliverToHangar"),
            ActionType::PutDownRack { .. } => write!(f, "PutDownRack"),
            ActionType::PickUpRack { .. } => write!(f, "PickUpRack"),
            ActionType::SwitchToNextBeluga => write!(f, "SwitchToNextBeluga"),
        }
    }
}

#[derive(Debug)]
pub struct Action {
    action_type: ActionType,
    presence: Lit,
    pub start: VarCst,
    task_id: TaskId,
}

impl Evaluable for Action {
    type Value = Op;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        if !solution.entails(self.presence) {
            // action is absent
            return None;
        }
        let op: String;
        match self.action_type {
            ActionType::LoadBeluga { j, b, t } => {
                op = format!(
                    "load_beluga(j:{}, b:{}, t:{})",
                    solution.eval(j).unwrap(),
                    solution.eval(b).unwrap(),
                    solution.eval(t).unwrap()
                )
            }
            ActionType::UnloadBeluga { j, b, t } => {
                op = format!(
                    "unload_beluga(j:{}, b:{}, t:{})",
                    solution.eval(j).unwrap(),
                    solution.eval(b).unwrap(),
                    solution.eval(t).unwrap()
                )
            }
            ActionType::GetFromHangar { j, h, t } => {
                op = format!(
                    "get_from_hangar(j:{}, h:{}, t:{})",
                    solution.eval(j).unwrap(),
                    solution.eval(h).unwrap(),
                    solution.eval(t).unwrap()
                )
            }
            ActionType::DeliverToHangar { j, h, t, pl } => {
                op = format!(
                    "deliver_to_hangar(j:{}, h:{}, t:{}, pl:{})",
                    solution.eval(j).unwrap(),
                    solution.eval(h).unwrap(),
                    solution.eval(t).unwrap(),
                    solution.eval(pl).unwrap()
                )
            }
            ActionType::PutDownRack { j, t, r, side } => {
                op = format!(
                    "put_down_rack(j:{}, t:{}, r:{}, s:{})",
                    solution.eval(j).unwrap(),
                    solution.eval(t).unwrap(),
                    solution.eval(r).unwrap(),
                    solution.eval(side).unwrap()
                )
            }
            ActionType::PickUpRack { j, t, r, side } => {
                op = format!(
                    "pick_up_rack(j:{}, t:{}, r:{}, s:{})",
                    solution.eval(j).unwrap(),
                    solution.eval(t).unwrap(),
                    solution.eval(r).unwrap(),
                    solution.eval(side).unwrap()
                )
            }
            ActionType::SwitchToNextBeluga => op = format!("switch_to_next_beluga()"),
        }
        Some(Op {
            start: solution.eval(self.start).unwrap(),
            task: self.task_id,
            op,
        })
    }
}

/// Represents an actual operation in the plan
#[derive(Debug)]
pub struct Op {
    pub start: IntCst,
    task: TaskId,
    op: String,
}

//use new_pick_up_rack(..) and new_put_down_rack(..) to create task that swap a jig from a rack to another
//the actions are then pushed into actions
//this time the actions are optionnal
pub fn add_swap_racks(
    side: Side,
    presence: Lit,
    model: &mut Sched,
    actions: &mut Vec<Action>,
    instance: &instance::Instance,
) {
    let start1 = model.new_opt_timepoint(presence);
    let start2 = model.new_opt_timepoint(presence);
    let task_id = model.add_task(Task {
        name: "swap_racks".to_string(),
        start: start1,
        end: start2 + 1,
        presence,
    });
    let param1 = states::ActParam {
        start: start1,
        end: start1 + 1,
        presence,
        source: Some(task_id),
    };
    let param2 = states::ActParam {
        start: start2,
        end: start2 + 1,
        presence,
        source: Some(task_id),
    };

    //variables
    let (lb, ub) = instance.bounds_rack();
    let r1 = model.new_optional_var(lb, ub, presence);
    let r2 = model.new_optional_var(lb, ub, presence);
    let (lb, ub) = instance.bounds_jig();
    let j = model.new_optional_var(lb, ub, presence);
    let t: Var = match side {
        Side::Beluga => get_empty_trailer_beluga(model, &param1, instance),
        Side::Factory => get_empty_trailer_factory(model, &param1, instance),
    };

    actions.push(new_pick_up_rack(
        j.into(),
        t.into(),
        r1.into(),
        side,
        model,
        &param1,
        instance,
    ));
    actions.push(new_put_down_rack(
        j.into(),
        t.into(),
        r2.into(),
        side,
        model,
        &param2,
        instance,
    ));

    //model.add_constraint(lt(start1, start2));
}

//use new_pick_up_rack(..), new_deliver_to_hangar(..), new_get_from_hangar(..) and new_put_down_rack(..) to create 2 tasks
//task 1 takes a jig from a rack to a hangar
//task 2 takes a jig from a hangar to a rack
//the actions are then pushed into actions
pub fn add_send_to_prod(
    j: JigId,
    pl: VarCst,
    start: VarCst,
    model: &mut Sched,
    actions: &mut Vec<Action>,
    instance: &instance::Instance,
) {
    //parameters
    let presence = Lit::TRUE;
    let start2: VarCst = model.new_opt_timepoint(presence);
    let start3: VarCst = model.new_opt_timepoint(presence);
    let start4: VarCst = model.new_opt_timepoint(presence);
    let task_1: TaskId = model.add_task(Task {
        name: "send_to_prod".to_string(),
        start,
        end: start2 + 1,
        presence,
    });
    let task_2: TaskId = model.add_task(Task {
        name: "send_to_prod".to_string(),
        start: start3,
        end: start4 + 1,
        presence,
    });
    let param = states::ActParam {
        start,
        end: start + 1,
        presence,
        source: Some(task_1),
    };
    let param2 = states::ActParam {
        start: start2,
        end: start2 + 1,
        presence,
        source: Some(task_1),
    };
    let param3 = states::ActParam {
        start: start3,
        end: start3 + 1,
        presence,
        source: Some(task_2),
    };
    let param4 = states::ActParam {
        start: start4,
        end: start4 + 1,
        presence,
        source: Some(task_2),
    };

    //variables
    let t1: Var = get_empty_trailer_factory(model, &param, instance);
    let t2: Var = get_empty_trailer_factory(model, &param3, instance);
    let (lb, ub) = instance.bounds_rack();
    let r1: Var = model.new_optional_var(lb as IntCst, ub as IntCst, presence);
    let r2: Var = model.new_optional_var(lb as IntCst, ub as IntCst, presence);
    let h: Var = get_empty_hangar(model, &param, instance);
    let side = Side::Factory;

    //Pick up rack
    actions.push(new_pick_up_rack(
        j.into(),
        t1.into(),
        r1.into(),
        side,
        model,
        &param,
        instance,
    ));

    //Deliver_to_hangar
    actions.push(new_deliver_to_hangar(
        j,
        h.into(),
        t1.into(),
        pl,
        model,
        &param2,
        instance,
    ));

    //Get from hangar
    actions.push(new_get_from_hangar(j.into(), h.into(), t2.into(), model, &param3));

    //Put down rack
    actions.push(new_put_down_rack(
        j.into(),
        t2.into(),
        r2.into(),
        side,
        model,
        &param4,
        instance,
    ));

    //precedence
    /* model.add_constraint(lt(start, start2));
    model.add_constraint(lt(start2, start3));
    model.add_constraint(lt(start3, start4)); */
}

//use new_unload_beluga(..) and new_put_down_rack(..) to create task that take a jig from an incoming beluga to a rack
//the actions are then pushed into actions
pub fn add_beluga_to_rack(
    j: VarCst,
    b: VarCst,
    start: VarCst,
    model: &mut Sched,
    actions: &mut Vec<Action>,
    instance: &instance::Instance,
) {
    //parameters
    let presence = Lit::TRUE;
    let end = start + 1;
    let start2: VarCst = model.new_opt_timepoint(presence);
    let end2 = start2 + 1;
    let task_id = model.add_task(Task {
        name: "beluga_to_rack".to_string(),
        start,
        end: end2,
        presence,
    });
    let param = states::ActParam {
        start,
        end,
        presence,
        source: Some(task_id),
    };
    let param2 = states::ActParam {
        start: start2,
        end: end2,
        presence,
        source: Some(task_id),
    };

    let t: Var = get_empty_trailer_beluga(model, &param, instance);
    let (lb, ub) = instance.bounds_rack();
    let r: Var = model.new_optional_var(lb as IntCst, ub as IntCst, presence);
    let side = Side::Beluga;

    //Unload_beluga
    actions.push(new_unload_beluga_action(j.into(), b.into(), t.into(), model, &param));

    //Put down rack
    actions.push(new_put_down_rack(
        j.into(),
        t.into(),
        r.into(),
        side,
        model,
        &param2,
        instance,
    ));

    //model.add_constraint(lt(start, start2));
}

//use new_pick_up_rack(..) and new_load_beluga(..) to create task that take a jig from a rack to a outgoing beluga
//the actions are then pushed into actions
pub fn add_rack_to_beluga(
    j_type: JigTypeId,
    b: VarCst,
    start: VarCst,
    model: &mut Sched,
    actions: &mut Vec<Action>,
    instance: &instance::Instance,
) {
    //parameters
    let presence = Lit::TRUE;
    let end = start + 1;
    let start2: VarCst = model.new_opt_timepoint(presence);
    let end2 = start2 + 1;
    let task_id = model.add_task(Task {
        name: "rack_to_beluga".to_string(),
        start,
        end: end2,
        presence,
    });
    let param = states::ActParam {
        start,
        end,
        presence,
        source: Some(task_id),
    };
    let param2 = states::ActParam {
        start: start2,
        end: end2,
        presence,
        source: Some(task_id),
    };

    //variables
    let j = get_jig_from_jigtype(j_type, model, &param, instance);
    let t: Var = get_empty_trailer_beluga(model, &param, instance);
    let (lb, ub) = instance.bounds_rack();
    let r: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let side = Side::Beluga;

    //Pick up rack
    actions.push(new_pick_up_rack(j.into(), t.into(), r, side, model, &param, instance));

    //Load beluga
    actions.push(new_load_beluga_action(j.into(), b, t.into(), model, &param2));

    //model.add_constraint(lt(start, start2));
}

//push a new switch_beluga action into actions
//b is the current beluga id
pub fn add_switch_to_next_beluga(
    b: BelugaId,
    start: VarCst,
    model: &mut Sched,
    actions: &mut Vec<Action>,
    instance: &instance::Instance,
) {
    //parameters
    let presence = Lit::TRUE;
    let end = start + 1;
    let task_id = model.add_task(Task {
        name: "switch_to_next_beluga".to_string(),
        start,
        end,
        presence,
    });
    let param = states::ActParam {
        start,
        end,
        presence,
        source: Some(task_id),
    };
    actions.push(new_switch_to_next_beluga(b, model, &param, instance));
}

pub fn new_load_beluga_action(j: VarCst, b: VarCst, t: VarCst, model: &mut Sched, param: &ActParam) -> Action {
    let pos: Var = get_current_value(next_pos_in_outgoing(), model, param);

    //EFFECTS
    //update j loc
    let new_j_loc = JigLoc {
        held_by: Some((JigHolder::Outgoing as usize).into()),
        num: Some(b.into()),
        pos: Some(pos.into()),
    };
    effect_on_jig_loc(j, &new_j_loc, model, param);

    //CONDITIONS
    //previous j loc
    let prev_j_loc = JigLoc {
        held_by: Some((JigHolder::TrailerBeluga as usize).into()),
        num: Some(t.into()),
        pos: Some(0.into()),
    };
    let prev_j_state = JigState {
        empty: Some((true as i32).into()),
        size: None,
    };
    cond_on_jig_loc(j, &prev_j_loc, model, param);
    cond_on_jig_state(j, &prev_j_state, model, param);

    //current_beluga == b
    model.add_constraint(HasValueAt {
        state_var: current_beluga(),
        value: b.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });

    Action {
        action_type: ActionType::LoadBeluga { j, b, t },
        presence: param.presence,
        start: param.start,
        task_id: param.source.unwrap(),
    }
}

pub fn new_unload_beluga_action(j: VarCst, b: VarCst, t: VarCst, model: &mut Sched, param: &ActParam) -> Action {
    //Var Pos = next_pos_in_flight
    let pos: Var = get_current_value(next_pos_in_incoming(), model, param);

    //EFFECTS
    let new_j_loc = JigLoc {
        held_by: Some((JigHolder::TrailerBeluga as usize).into()),
        num: Some(t.into()),
        pos: Some(0.into()),
    };
    effect_on_jig_loc(j, &new_j_loc, model, &param);
    //Decrement the pos in incoming

    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: next_pos_in_incoming(),
        operation: EffectOp::Step((-1).into()),
        prez: param.presence,
        source: param.source,
    });

    //CONDITIONS
    //prev j loc
    let prev_j_loc = JigLoc {
        held_by: Some((JigHolder::Incoming as usize).into()),
        num: Some(b.into()),
        pos: Some(pos.into()),
    };
    cond_on_jig_loc(j, &prev_j_loc, model, &param);

    //current_beluga == b
    model.add_constraint(HasValueAt {
        state_var: current_beluga(),
        value: b.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });

    Action {
        action_type: ActionType::UnloadBeluga { j, b, t },
        presence: param.presence,
        start: param.start,
        task_id: param.source.unwrap(),
    }
}

pub fn new_put_down_rack(
    j: VarCst,
    t: VarCst,
    r: VarCst,
    side: Side,
    model: &mut Sched,
    param: &ActParam,
    instance: &instance::Instance,
) -> Action {
    let size_of_j: Var = get_current_value(jig_state(j, JigStateAttr::Size), model, param);
    let pos: VarCst;

    match side {
        Side::Beluga => {
            //append j at the beginning of the rack
            pos = 0.into();
            //shift pos of other jigs on rack r
            for j in 0..instance.jigs.len() {
                let prez: Lit = model.new_bool_var();
                let (holder, num) = j_is_in(j.into(), model, param, instance);
                let same_holder = model.reify(eq(holder, JigHolder::Rack as i32).into());
                let same_num = model.reify(eq(num, r).into());
                let condition: Lit = model.reify(CoreExpr::And(and([same_holder, same_num])));
                model.add_constraint(implies(condition, prez));
                //optional effect : only if on same rack
                let mutex_end = model.new_timepoint();
                model.add_effect(Effect {
                    transition_start: param.start,
                    transition_end: param.end,
                    mutex_end,
                    state_var: jig_loc(j, JigLocAttr::Pos),
                    operation: EffectOp::Step(1.into()),
                    prez: prez,
                    source: param.source,
                });
            }
        }
        Side::Factory => {
            //append j at the end of the rack
            pos = VarCst {
                var: get_current_value(last_pos_on_rack(r), model, param).into(),
                shift: 1,
            }
        }
    }

    //EFFECTS
    let new_j_loc = JigLoc {
        held_by: Some((JigHolder::Rack as usize).into()),
        num: Some(r.into()),
        pos: Some(pos.into()),
    };
    effect_on_jig_loc(j, &new_j_loc, model, param);

    //reduce free space on rack
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: free_space_on_rack(r),
        operation: EffectOp::Step((-size_of_j).into()),
        prez: param.presence,
        source: param.source,
    });

    //update last_pos_on_rack
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: last_pos_on_rack(r),
        operation: EffectOp::Step(1.into()),
        prez: param.presence,
        source: param.source,
    });

    //CONDITIONS
    let held_by: Option<LinTerm> = match side {
        Side::Beluga => Some((JigHolder::TrailerBeluga as usize).into()),
        Side::Factory => Some((JigHolder::TrailerFactory as usize).into()),
    };
    let prev_j_loc = JigLoc {
        held_by,
        num: Some(t.into()),
        pos: Some(0.into()),
    };
    cond_on_jig_loc(j, &prev_j_loc, model, param);

    //enough free space
    let free_space = get_current_value(free_space_on_rack(r), model, param);
    model.add_constraint(leq(size_of_j, free_space));

    Action {
        action_type: ActionType::PutDownRack {
            j: j.into(),
            t,
            r,
            side: (side as usize).into(),
        },
        presence: param.presence,
        start: param.start,
        task_id: param.source.unwrap(),
    }
}

pub fn new_pick_up_rack(
    j: VarCst,
    t: VarCst,
    r: VarCst,
    side: Side,
    model: &mut Sched,
    param: &ActParam,
    instance: &instance::Instance,
) -> Action {
    let prev_pos: VarCst;
    match side {
        Side::Beluga => {
            //j was at the beginning of the rack
            prev_pos = 0.into();
            //shift pos of other jigs on rack r
            for other_j in 0..instance.jigs.len() {
                let prez: Lit = model.new_bool_var();
                let (holder, num) = j_is_in(other_j.into(), model, param, instance);
                let same_holder = model.reify(eq(holder, JigHolder::Rack as i32).into());
                let same_num = model.reify(eq(num, r).into());
                let diff_j = model.reify(neq(j, other_j as i32).into());
                let condition: Lit = model.reify(CoreExpr::And(and([same_holder, same_num, diff_j])));
                model.add_constraint(implies(condition, prez));
                //optional effect : only if on same rack
                let mutex_end = model.new_timepoint();
                model.add_effect(Effect {
                    transition_start: param.start,
                    transition_end: param.end,
                    mutex_end,
                    state_var: jig_loc(other_j, JigLocAttr::Pos),
                    operation: EffectOp::Step((-1).into()),
                    prez: prez,
                    source: param.source,
                });
            }
        }
        Side::Factory => {
            //j was at the end of the rack
            prev_pos = get_current_value(last_pos_on_rack(r), model, param).into();
        }
    }

    //EFFECTS
    let held_by: Option<LinTerm> = match side {
        Side::Beluga => Some((JigHolder::TrailerBeluga as usize).into()),
        Side::Factory => Some((JigHolder::TrailerFactory as usize).into()),
    };
    let new_j_loc = JigLoc {
        held_by,
        num: Some(t.into()),
        pos: Some(0.into()),
    };
    effect_on_jig_loc(j.into(), &new_j_loc, model, param);

    //release free space on rack
    let size_of_j: Var = get_current_value(jig_state(j, JigStateAttr::Size), model, param);
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: free_space_on_rack(r),
        operation: EffectOp::Step(size_of_j.into()),
        prez: param.presence,
        source: param.source,
    });

    //update last_pos_on_rack
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: last_pos_on_rack(r),
        operation: EffectOp::Step((-1).into()),
        prez: param.presence,
        source: param.source,
    });

    //CONDITIONS
    let prev_j_loc = JigLoc {
        held_by: Some((JigHolder::Rack as usize).into()),
        num: Some(r.into()),
        pos: Some(prev_pos.into()),
    };
    cond_on_jig_loc(j.into(), &prev_j_loc, model, param);

    Action {
        action_type: ActionType::PickUpRack {
            j,
            t,
            r,
            side: (side as usize).into(),
        },
        presence: param.presence,
        start: param.start,
        task_id: param.source.unwrap(),
    }
}

// The j is a JigId and not a VarCst because we need to access its new size now that it's empty
// That's also why we need the instance
pub fn new_deliver_to_hangar(
    j: JigId,
    h: VarCst,
    t: VarCst,
    pl: VarCst,
    model: &mut Sched,
    param: &ActParam,
    instance: &instance::Instance,
) -> Action {
    //EFFECTS
    let new_j_loc = JigLoc {
        held_by: Some((JigHolder::Hangar as usize).into()),
        num: Some(h.into()),
        pos: Some(0.into()),
    };
    let new_j_state = JigState {
        empty: Some((true as usize).into()),
        size: Some(instance.size_of_jig(j, true).unwrap().into()),
    };
    effect_on_jig_loc(j.into(), &new_j_loc, model, param);
    effect_on_jig_state(j.into(), &new_j_state, model, param);

    //CONDITIONS
    let prev_j_loc = JigLoc {
        held_by: Some((JigHolder::TrailerFactory as usize).into()),
        num: Some(t.into()),
        pos: Some(0.into()),
    };
    let prev_j_state = JigState {
        empty: Some((false as usize).into()),
        size: None,
    };
    cond_on_jig_loc(j.into(), &prev_j_loc, model, param);
    cond_on_jig_state(j.into(), &prev_j_state, model, param);

    Action {
        action_type: ActionType::DeliverToHangar { j: j.into(), h, t, pl },
        presence: param.presence,
        start: param.start,
        task_id: param.source.unwrap(),
    }
}

pub fn new_get_from_hangar(j: VarCst, h: VarCst, t: VarCst, model: &mut Sched, param: &ActParam) -> Action {
    //EFFECTS
    let new_j_loc = JigLoc {
        held_by: Some((JigHolder::TrailerFactory as usize).into()),
        num: Some(t.into()),
        pos: Some(0.into()),
    };
    effect_on_jig_loc(j, &new_j_loc, model, param);

    //CONDITIONS
    let prev_j_loc = JigLoc {
        held_by: Some((JigHolder::Hangar as usize).into()),
        num: Some(h.into()),
        pos: Some(0.into()),
    };
    cond_on_jig_loc(j, &prev_j_loc, model, param);

    Action {
        action_type: ActionType::GetFromHangar { j, h, t },
        presence: param.presence,
        start: param.start,
        task_id: param.source.unwrap(),
    }
}

pub fn new_switch_to_next_beluga(
    b: BelugaId,
    model: &mut Sched,
    param: &ActParam,
    instance: &instance::Instance,
) -> Action {
    //EFFECTS
    //current_beluga ++
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: current_beluga(),
        operation: EffectOp::Step(1.into()),
        prez: param.presence,
        source: param.source,
    });
    //reset the position
    if b + 1 < instance.flights.len() {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: param.start,
            transition_end: param.end,
            mutex_end,
            state_var: next_pos_in_incoming(),
            operation: EffectOp::Assign((instance.flights[b + 1].incoming.len() as i32 - 1).into()),
            prez: param.presence,
            source: param.source,
        });
    }
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: next_pos_in_outgoing(),
        operation: EffectOp::Assign(0.into()),
        prez: param.presence,
        source: param.source,
    });

    //CONDITIONS
    model.add_constraint(HasValueAt {
        state_var: current_beluga(),
        value: b.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });

    Action {
        action_type: ActionType::SwitchToNextBeluga,
        presence: param.presence,
        start: param.start,
        task_id: param.source.unwrap(),
    }
}
