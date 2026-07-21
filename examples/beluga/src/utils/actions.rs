use std::fmt;

use aries_solver::core::state::Evaluable;
use aries_solver::prelude::*;
use aries_solver::{lang::ModelView};
use aries_timelines::{constraints::HasValueAt, *};

use super::*;
use super::states::*;
use super::instance::JigHolder;

pub enum ActionType {
    LoadBeluga { j : VarCst, b : VarCst, t : VarCst},
    UnloadBeluga { j : VarCst, b : VarCst, t : VarCst},
    GetFromHangar { j : VarCst, h : VarCst, t : VarCst},
    DeliverToHangar { j : VarCst, h : VarCst, t : VarCst, pl : VarCst},
    PutDownRack { j : VarCst, t : VarCst, r : VarCst, side : VarCst},
    PickUpRack { j : VarCst, t : VarCst, r : VarCst, side : VarCst},
    SwitchToNextBeluga,
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionType::LoadBeluga {..} => write!(f, "LoadBeluga"),
            ActionType::UnloadBeluga {..} => write!(f, "UnloadBeluga"),
            ActionType::GetFromHangar {..} => write!(f, "GetFromHangar"),
            ActionType::DeliverToHangar {..} => write!(f, "DeliverToHangar"),
            ActionType::PutDownRack {..} => write!(f, "PutDownRack"),
            ActionType::PickUpRack {..} => write!(f, "PickUpRack"),
            ActionType::SwitchToNextBeluga => write!(f, "SwitchToNextBeluga"),
        }
    }
}

pub struct Action {
    action_type : ActionType,
    presence : Lit,
    pub start : VarCst,
    task_id : TaskId,
}

impl Evaluable for Action {
    type Value = Op;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        if !solution.entails(self.presence) {
            // action is absent
            return None;
        }
        let op : String ;
        match self.action_type {
            ActionType::LoadBeluga { j, b, t } => op = format!("load_beluga({}, {}, {})", solution.eval(j).unwrap(), solution.eval(b).unwrap(), solution.eval(t).unwrap()),
            ActionType::UnloadBeluga { j, b, t } => op = format!("unload_beluga({}, {}, {})", solution.eval(j).unwrap(), solution.eval(b).unwrap(), solution.eval(t).unwrap()),
            ActionType::GetFromHangar { j, h, t } => op = format!("get_from_hangar({}, {}, {})", solution.eval(j).unwrap(), solution.eval(h).unwrap(), solution.eval(t).unwrap()),
            ActionType::DeliverToHangar { j, h, t, pl } => op = format!("deliver_to_hangar({}, {}, {}, {})", solution.eval(j).unwrap(), solution.eval(h).unwrap(), solution.eval(t).unwrap(), solution.eval(pl).unwrap()),
            ActionType::PutDownRack { j, t, r, side } => op = format!("put_down_rack({}, {}, {}, {})", solution.eval(j).unwrap(), solution.eval(t).unwrap(), solution.eval(r).unwrap(), solution.eval(side).unwrap()),
            ActionType::PickUpRack { j, t, r, side } => op = format!("pick_up_rack({}, {}, {}, {})", solution.eval(j).unwrap(), solution.eval(t).unwrap(), solution.eval(r).unwrap(), solution.eval(side).unwrap()),
            ActionType::SwitchToNextBeluga => op = format!("switch_to_next_beluga()"),
        }
        Some(Op {
            start: solution.eval(self.start).unwrap(),
            op
        })
    }
}

/// Represents an actual operation in the plan
#[derive(Debug)]
pub struct Op {
    pub start: IntCst,
    op: String,
}


//use new_pick_up_rack(..), new_deliver_to_hangar(..), new_get_from_hangar(..) and new_put_down_rack(..) to create task that take a jig from a rack to a hangar
//the actions are then pushed into actions
pub fn add_send_to_prod(j : VarCst, pl :VarCst, start : VarCst, model : &mut Sched, actions : &mut Vec<Action>, instance : &instance::Instance) {
    //parameters
    let presence = Lit::TRUE;
    let start2 : VarCst = model.new_opt_timepoint(presence);
    let start3 : VarCst = model.new_opt_timepoint(presence);
    let start4 : VarCst = model.new_opt_timepoint(presence);
    let task_id = model.add_task(Task {
        name: "send_to_prod".to_string(),
        start,
        end : start2 + 1,
        presence,
    });
    let param = states::ActParam{
        start,
        end : start + 1,
        presence,
        source : Some(task_id)
    };
    let param2 = states::ActParam{
        start : start2,
        end : start2 + 1,
        presence,
        source : Some(task_id)
    };
    let param3 = states::ActParam{
        start : start3,
        end : start3 + 1,
        presence,
        source : Some(task_id)
    };
    let param4 = states::ActParam{
        start : start4,
        end : start4 + 1,
        presence,
        source : Some(task_id)
    };

    //variables
    let (lb, ub) = instance.bounds_trailer();
    let t1: VarCst = model.new_optional_var(lb, ub, presence).into();
    let t2: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let (lb, ub) = instance.bounds_rack();
    let r1: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let r2: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let (lb, ub) = instance.bounds_hangar();
    let h: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let side : VarCst = (Side::Factory as usize).into();

    //Pick up rack
    actions.push(new_pick_up_rack(j, t1, r1, side, model, &param));

    //Deliver_to_hangar
    actions.push(new_deliver_to_hangar(j, h, t1, pl, model, &param2));

    //Get from hangar
    actions.push(new_get_from_hangar(j, h, t2, model, &param3));

    //Put down rack
    actions.push(new_put_down_rack(j, t2, r2, side, model, &param4));

    //precedence
    model.add_constraint(lt(start, start2));
    model.add_constraint(lt(start2, start3));
    model.add_constraint(lt(start3, start4));

}

//use new_unload_beluga(..) and new_put_down_rack(..) to create task that take a jig from an incoming beluga to a rack
//the actions are then pushed into actions
pub fn add_beluga_to_rack(j : VarCst, b : VarCst, start : VarCst, model : &mut Sched, actions : &mut Vec<Action>, instance : &instance::Instance) {
    //parameters
    let presence = Lit::TRUE;
    let end = start + 1;
    let start2 : VarCst = model.new_opt_timepoint(presence);
    let end2 = start2 + 1;
    let task_id = model.add_task(Task {
        name: "beluga_to_rack".to_string(),
        start,
        end : end2,
        presence,
    });
    let param = states::ActParam{
        start,
        end,
        presence,
        source : Some(task_id)
    };
    let param2 = states::ActParam{
        start : start2,
        end : end2,
        presence,
        source : Some(task_id)
    };

    //variables
    let (lb, ub) = instance.bounds_trailer();
    let t: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let (lb, ub) = instance.bounds_rack();
    let r: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let side = Side::Beluga as usize;

    //Unload_beluga
    actions.push(new_unload_beluga_action(j.into(), b.into(), t, model, &param, instance));

    //Put down rack
    actions.push(new_put_down_rack(j.into(), t, r, side.into(), model, &param2));

    model.add_constraint(lt(start, start2));

}

//use new_pick_up_rack(..) and new_load_beluga(..) to create task that take a jig from a rack to a outgoing beluga
//the actions are then pushed into actions
pub fn add_rack_to_beluga(j : VarCst, b : VarCst, start : VarCst, model : &mut Sched, actions : &mut Vec<Action>, instance : &instance::Instance) {
    //parameters
    let presence = Lit::TRUE;
    let end = start + 1;
    let start2 : VarCst = model.new_opt_timepoint(presence);
    let end2 = start2 + 1;
    let task_id = model.add_task(Task {
        name: "rack_to_beluga".to_string(),
        start,
        end : end2,
        presence,
    });
    let param = states::ActParam{
        start,
        end,
        presence,
        source : Some(task_id)
    };
    let param2 = states::ActParam{
        start : start2,
        end : end2,
        presence,
        source : Some(task_id)
    };

    //variables
    let t: VarCst = get_empty_trailer(model, &param, instance);
    let (lb, ub) = instance.bounds_rack();
    let r: VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, presence).into();
    let side = Side::Beluga as usize;

    //Pick up rack
    actions.push(new_pick_up_rack(j, t, r, side.into(), model, &param));

    //Load beluga
    actions.push(new_load_beluga_action(j, b, t, model, &param2, instance));

    model.add_constraint(lt(start, start2));

}

pub fn new_load_beluga_action(j : VarCst, b : VarCst, t : VarCst, model : &mut Sched, param : &ActParam, instance: &instance::Instance) -> Action {

    //Var Pos = next_pos_in_flight
    let (lb, ub) = instance.bounds_outgoing();
    let pos : VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, param.presence).into();
    model.add_constraint(HasValueAt {
        state_var: next_pos_in_outgoing(),
        value: pos.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });

    //EFFECTS
    //update j state
    let new_jig_state = JigState {
        held_by : Some((JigHolder::Outgoing as usize).into()),
        num : Some(b.into()),
        pos : Some(pos.into()),
        empty : None,
    };
    effect_on_jig_state(j, &new_jig_state, model, param);

    //conditions
    //previous j state
    let prev_jig_state = JigState {
        held_by : Some((JigHolder::Trailer as usize).into()),
        num : Some(t.into()),
        pos : Some(0.into()),
        empty : None,
    };
    cond_on_jig_state(j, &prev_jig_state, model, param);
    //current_beluga = b
    model.add_constraint(HasValueAt {
        state_var: current_beluga(),
        value: b.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });

    Action {
    action_type : ActionType::LoadBeluga { j, b, t },
    presence : param.presence,
    start : param.start,
    task_id : param.source.unwrap(),
}
}

pub fn new_unload_beluga_action(j : VarCst, b : VarCst, t : VarCst, model : &mut Sched, param : &ActParam, instance : &instance::Instance) -> Action {

    //Var Pos = next_pos_in_flight
    let (lb, ub) = instance.bounds_incoming();
    let pos : VarCst = model.new_optional_var(lb as IntCst, ub as IntCst, param.presence).into();
    model.add_constraint(HasValueAt {
        state_var: next_pos_in_incoming(),
        value: pos.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });

    //EFFECTS
    let new_j_state = JigState {
        held_by : Some((JigHolder::Trailer as usize).into()),
        num : Some(t.into()),
        pos : Some(0.into()),
        empty : None,
    };
    effect_on_jig_state(j, &new_j_state, model, &param);
    //Decrement the pos in incoming
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: next_pos_in_incoming(),
        operation: EffectOp::Step((-1).into()),
        prez : param.presence,
        source: param.source,
    });



    //CONDITIONS
    //prev j state
    let prev_j_state = JigState {
        held_by : Some((JigHolder::Incoming as usize).into()),
        num : Some(b.into()),
        pos : Some(pos.into()),
        empty : None,
    };
    cond_on_jig_state(j, &prev_j_state, model, &param);

    //current_beluga == b
    model.add_constraint(HasValueAt {
        state_var: current_beluga(),
        value: b.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });

    Action {
        action_type : ActionType::UnloadBeluga { j, b, t },
        presence : param.presence,
        start : param.start,
        task_id : param.source.unwrap(),
    }
}

pub fn new_put_down_rack(j : VarCst, t : VarCst, r : VarCst, side : VarCst, model : &mut Sched, param : &ActParam) -> Action {
    //EFFECTS
    let new_j_state = JigState {
        held_by : Some((JigHolder::Rack as usize).into()),
        num : Some(r.into()),
        pos : Some(0.into()),
        empty : None,
    };
    effect_on_jig_state(j, &new_j_state, model, param);

    //CONDITIONS
    let prev_j_state = JigState {
        held_by : Some((JigHolder::Trailer as usize).into()),
        num : Some(t.into()),
        pos : Some(0.into()),
        empty : None,
    };
    cond_on_jig_state(j, &prev_j_state, model, param);

    Action {
        action_type : ActionType::PutDownRack { j, t, r, side },
        presence : param.presence,
        start : param.start,
        task_id : param.source.unwrap(),
    }
}

pub fn new_pick_up_rack(j : VarCst, t : VarCst, r : VarCst, side : VarCst, model : &mut Sched, param : &ActParam) -> Action {
    //EFFECTS
    let new_j_state = JigState {
        held_by : Some((JigHolder::Trailer as usize).into()),
        num : Some(t.into()),
        pos : Some(0.into()),
        empty : None,
    };
    effect_on_jig_state(j, &new_j_state, model, param);

    //CONDITIONS
    let prev_j_state = JigState {
        held_by : Some((JigHolder::Rack as usize).into()),
        num : Some(r.into()),
        pos : None,
        empty : None,
    };
    cond_on_jig_state(j, &prev_j_state, model, param);

    Action {
        action_type : ActionType::PickUpRack { j, t, r, side },
        presence : param.presence,
        start : param.start,
        task_id : param.source.unwrap(),
    }
}

fn new_deliver_to_hangar(j : VarCst, h : VarCst, t : VarCst, pl : VarCst, model : &mut Sched, param : &ActParam) -> Action {
    //EFFECTS
    let new_j_state = JigState {
        held_by : Some((JigHolder::Hangar as usize).into()),
        num : Some(h.into()),
        pos : Some(0.into()),
        empty : Some((true as usize).into())
    };
    effect_on_jig_state(j, &new_j_state, model, param);

    //One step of the pl is completed
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: param.start,
        transition_end: param.end,
        mutex_end,
        state_var: prod_state(pl),
        operation: EffectOp::Step((1).into()),
        prez : param.presence,
        source: param.source,
    });

    //CONDITIONS
    let prev_j_state = JigState {
        held_by : Some((JigHolder::Trailer as usize).into()),
        num : Some(t.into()),
        pos : Some(0.into()),
        empty : None
    };
    cond_on_jig_state(j, &prev_j_state, model, param);

    Action {
        action_type : ActionType::DeliverToHangar { j, h, t, pl },
        presence : param.presence,
        start : param.start,
        task_id : param.source.unwrap(),
    }
}

fn new_get_from_hangar(j : VarCst, h : VarCst, t : VarCst, model : &mut Sched, param : &ActParam) -> Action {
    //EFFECTS
    let new_j_state = JigState {
        held_by : Some((JigHolder::Trailer as usize).into()),
        num : Some(t.into()),
        pos : Some(0.into()),
        empty : None
    };
    effect_on_jig_state(j, &new_j_state, model, param);

    //CONDITIONS
    let prev_j_state = JigState {
        held_by : Some((JigHolder::Hangar as usize).into()),
        num : Some(h.into()),
        pos : Some(0.into()),
        empty : None
    };
    cond_on_jig_state(j, &prev_j_state, model, param);

    Action {
        action_type : ActionType::GetFromHangar { j, h, t },
        presence : param.presence,
        start : param.start,
        task_id : param.source.unwrap(),
    }
}