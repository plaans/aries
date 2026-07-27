use aries_solver::{lang::ModelView, prelude::*};
use aries_timelines::StateVar;
use aries_timelines::{constraints::HasValueAt, *};

use super::*;
use std::clone::Clone;

pub enum JigLocAttr {
    HeldBy = 0,
    Number = 1,
    Pos = 2,
}

pub enum JigStateAttr {
    Empty = 0,
    Size = 1,
}


#[derive(Debug, Copy, Clone)]
pub enum JigHolder {
    Incoming = 0,
    Outgoing = 1,
    Rack = 2,
    Hangar = 3,
    TrailerBeluga = 4,
    TrailerFactory = 5,
}


//STATEVARS

pub fn current_beluga() -> StateVar {
    StateVar {
        fluent: "current_beluga".to_string(),
        args: vec![],
    }
}

pub fn jig_loc(j: impl Into<LinTerm>, attr: JigLocAttr) -> StateVar {
    StateVar {
        fluent: "jig_loc".to_string(),
        args: vec![j.into(), (attr as usize).into()],
    }
}

pub fn jig_state(j: impl Into<LinTerm>, attr: JigStateAttr) -> StateVar {
    StateVar {
        fluent: "size_of_jig".to_string(),
        args: vec![j.into(), (attr as usize).into()],
    }
}

//when new flight arrives, is flight.incoming.len()-1
//decrements each time a new jig is unloaded
//When the flight departs, is -1 (no more jig to unload)
pub fn next_pos_in_incoming() -> StateVar {
    StateVar {
        fluent: "next_pos_in_incoming".to_string(),
        args: vec![],
    }
}

//when new flight arrives, is 0
//increment each time a new jig is loaded
//When the flight departs, is flight.outgoing.len() (all jigs are loaded)
pub fn next_pos_in_outgoing() -> StateVar {
    StateVar {
        fluent: "next_pos_in_outgoing".to_string(),
        args: vec![],
    }
}

//is incremented everytime we push a jig on the back (factory side)
//is incremented everytime we pop a jig from the back (factory side)
pub fn last_pos_on_rack(r: impl Into<LinTerm>) -> StateVar {
    StateVar {
        fluent: "last_pos_on_rack".to_string(),
        args: vec![r.into()],
    }
}
//is decremented everytime we push a jig on the front (beluga side)
//is incremented everytime we pop a jig from the front (beluga side)
pub fn first_pos_on_rack(r: impl Into<LinTerm>) -> StateVar {
    StateVar {
        fluent: "first_pos_on_rack".to_string(),
        args: vec![r.into()],
    }
}

pub fn free_space_on_rack(r: impl Into<LinTerm>) -> StateVar {
    StateVar {
        fluent: "free_space_on_rack".to_string(),
        args: vec![r.into()],
    }
}

pub fn is_empty(holder : JigHolder, num : impl Into<LinTerm>) -> StateVar {
    StateVar {
        fluent: "is_empty".to_string(),
        args: vec![(holder as usize).into(), num.into()],
    }
}

//Attribute 'end' is only used when creating effects, not constraints
pub struct EffectOrCondParam {
    pub start: VarCst,
    pub end: VarCst,
    pub presence: Lit,
    pub source: Option<TaskId>,
}

//get a variable representing the value of a statevar at time = param.start
pub fn get_current_value(statevar: StateVar, model: &mut Sched, param: &EffectOrCondParam) -> Var {
    let value: Var = model.new_optional_var(INT_CST_MIN, INT_CST_MAX - 1, param.presence);
    model.add_constraint(HasValueAt {
        state_var: statevar,
        value: value.into(),
        timepoint: param.start,
        prez: param.presence,
        source: param.source,
    });
    value
}

pub struct JigLoc {
    pub held_by: Option<IntTerm>,
    pub num: Option<IntTerm>,
    pub pos: Option<IntTerm>,
}

pub struct JigState {
    pub empty: Option<IntTerm>,
    pub size: Option<IntTerm>,
}


pub fn effect_on_jig_loc(j: VarCst, new_loc: &JigLoc, model: &mut Sched, param: &EffectOrCondParam) {
    //heldby
    if let Some(holder) = new_loc.held_by {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: param.start,
            transition_end: param.end,
            mutex_end,
            state_var: jig_loc(j, JigLocAttr::HeldBy),
            operation: EffectOp::Assign(holder),
            prez: param.presence,
            source: param.source,
        });
    }
    //number
    if let Some(num) = new_loc.num {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: param.start,
            transition_end: param.end,
            mutex_end,
            state_var: jig_loc(j, JigLocAttr::Number),
            operation: EffectOp::Assign(num),
            prez: param.presence,
            source: param.source,
        });
    }
    //pos
    if let Some(pos) = new_loc.pos {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: param.start,
            transition_end: param.end,
            mutex_end,
            state_var: jig_loc(j, JigLocAttr::Pos),
            operation: EffectOp::Assign(pos),
            prez: param.presence,
            source: param.source,
        });
    }
}

pub fn cond_on_jig_loc(j: VarCst, expected_loc: &JigLoc, model: &mut Sched, param: &EffectOrCondParam) {
    //heldby
    if let Some(holder) = expected_loc.held_by {
        model.add_constraint(HasValueAt {
            state_var: jig_loc(j, JigLocAttr::HeldBy),
            value: holder,
            timepoint: param.start,
            prez: param.presence,
            source: param.source,
        });
    }
    //number
    if let Some(num) = expected_loc.num {
        model.add_constraint(HasValueAt {
            state_var: jig_loc(j, JigLocAttr::Number),
            value: num,
            timepoint: param.start,
            prez: param.presence,
            source: param.source,
        });
    }
    //pos
    if let Some(pos) = expected_loc.pos {
        model.add_constraint(HasValueAt {
            state_var: jig_loc(j, JigLocAttr::Pos),
            value: pos,
            timepoint: param.start,
            prez: param.presence,
            source: param.source,
        });
    }
}

pub fn effect_on_jig_state(j: VarCst, new_state: &JigState, model: &mut Sched, param: &EffectOrCondParam) {
    //empty
    if let Some(empty) = new_state.empty {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: param.start,
            transition_end: param.end,
            mutex_end,
            state_var: jig_state(j, JigStateAttr::Empty),
            operation: EffectOp::Assign(empty),
            prez: param.presence,
            source: param.source,
        });
    }
    //size
    if let Some(size) = new_state.size {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: param.start,
            transition_end: param.end,
            mutex_end,
            state_var: jig_state(j, JigStateAttr::Size),
            operation: EffectOp::Assign(size),
            prez: param.presence,
            source: param.source,
        });
    }
}

pub fn cond_on_jig_state(j: VarCst, expected_state: &JigState, model: &mut Sched, param: &EffectOrCondParam) {
    //empty
    if let Some(empty) = expected_state.empty {
        model.add_constraint(HasValueAt {
            state_var: jig_state(j, JigStateAttr::Empty),
            value: empty,
            timepoint: param.start,
            prez: param.presence,
            source: param.source,
        });
    }
    //size
    if let Some(size) = expected_state.size {
        model.add_constraint(HasValueAt {
            state_var: jig_state(j, JigStateAttr::Size),
            value: size,
            timepoint: param.start,
            prez: param.presence,
            source: param.source,
        });
    }
}

pub fn get_jig_from_jigtype(
    j_type: JigTypeId,
    model: &mut Sched,
    param: &EffectOrCondParam,
    instance: &instance::Instance,
) -> Var {
    let (lb, ub) = instance.bounds_jig();
    let j_var: Var = model.new_optional_var(lb, ub, param.presence);
    let mut disjuncts: Vec<Lit> = vec![];
    for (other_j, jig) in instance.jigs.iter().enumerate() {
        if jig.jig_type == j_type {
            disjuncts.push(model.reify(eq(j_var, other_j as i32).into()));
        }
    }
    model.add_constraint(or(disjuncts));
    j_var
}

//gives current holder and num of j
pub fn j_is_in(j: VarCst, model: &mut Sched, param: &EffectOrCondParam, instance: &instance::Instance) -> (Var, Var) {
    let holder = model.new_optional_var(0, 5, param.presence);
    let (lb, ub) = instance.bounds_jig_holder();
    let num = model.new_optional_var(lb, ub, param.presence);
    let j_loc = JigLoc {
        held_by: Some(holder.into()),
        num: Some(num.into()),
        pos: None,
    };
    cond_on_jig_loc(j.into(), &j_loc, model, param);
    (holder, num)
}

pub fn set_initial_state(model: &mut Sched, instance: &instance::Instance) {
    //current_beluga() = 0 at origin
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: model.origin,
        transition_end: model.origin,
        mutex_end,
        state_var: current_beluga(),
        operation: EffectOp::Assign(0.into()),
        prez: Lit::TRUE,
        source: None,
    });
    //Set jig initial state
    let initial_param = EffectOrCondParam {
        start: model.origin,
        end: model.origin,
        presence: Lit::TRUE,
        source: None,
    };
    for (b, flight) in instance.flights.iter().enumerate() {
        let mut flight_in_order = flight.incoming.clone();
        flight_in_order.reverse();
        for (pos, &j) in flight_in_order.iter().enumerate() {
            let initial_j_loc = JigLoc {
                held_by: Some((JigHolder::Incoming as usize).into()),
                num: Some(b.into()),
                pos: Some(pos.into()),
            };
            let empty = instance.jigs[j].empty;
            let size = instance.size_of_jig(j, empty).unwrap();
            let initial_j_state = JigState {
                empty: Some((empty as i32).into()),
                size: Some(size.into()),
            };
            effect_on_jig_loc(j.into(), &initial_j_loc, model, &initial_param);
            effect_on_jig_state(j.into(), &initial_j_state, model, &initial_param);
        }
    }
    //jigs on rack
    for (r, rack) in instance.racks.iter().enumerate() {
        let mut sum_size = 0;
        for (pos, &j) in rack.jigs.iter().enumerate() {
            let initial_j_loc = JigLoc {
                held_by: Some((JigHolder::Rack as i32).into()),
                num: Some(r.into()),
                pos: Some(pos.into()),
            };
            let empty = instance.jigs[j].empty;
            let size = instance.size_of_jig(j, empty).unwrap();
            let initial_j_state = JigState {
                empty: Some((empty as i32).into()),
                size: Some(size.into()),
            };
            effect_on_jig_loc(j.into(), &initial_j_loc, model, &initial_param);
            effect_on_jig_state(j.into(), &initial_j_state, model, &initial_param);
            sum_size += size;
        }
        //set initial free_space_on_rack
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin,
            transition_end: model.origin,
            mutex_end,
            state_var: free_space_on_rack(r),
            operation: EffectOp::Assign((rack.size - sum_size).into()),
            prez: Lit::TRUE,
            source: None,
        });
        //set initial last_pos_on_rack
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin,
            transition_end: model.origin,
            mutex_end,
            state_var: last_pos_on_rack(r),
            operation: EffectOp::Assign(((rack.jigs.len() as i32) - 1).into()),
            prez: Lit::TRUE,
            source: None,
        });
        //set initial first_pos_on_rack
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin,
            transition_end: model.origin,
            mutex_end,
            state_var: first_pos_on_rack(r),
            operation: EffectOp::Assign(0.into()),
            prez: Lit::TRUE,
            source: None,
        });
    }
    //Set next pos in flights
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: model.origin,
        transition_end: model.origin,
        mutex_end,
        state_var: next_pos_in_incoming(),
        operation: EffectOp::Assign((instance.flights[0].incoming.len() as i32 - 1).into()),
        prez: Lit::TRUE,
        source: None,
    });
    let mutex_end = model.new_timepoint();
    model.add_effect(Effect {
        transition_start: model.origin,
        transition_end: model.origin,
        mutex_end,
        state_var: next_pos_in_outgoing(),
        operation: EffectOp::Assign(0.into()),
        prez: Lit::TRUE,
        source: None,
    });
    //Set is_empty to true
    for t in 0..instance.trailers_beluga.len() {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin,
            transition_end: model.origin,
            mutex_end,
            state_var: is_empty(JigHolder::TrailerBeluga, t),
            operation: EffectOp::Assign((true as usize).into()),
            prez: Lit::TRUE,
            source: None,
        });
    }
    for t in 0..instance.trailers_factory.len() {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin,
            transition_end: model.origin,
            mutex_end,
            state_var: is_empty(JigHolder::TrailerFactory, t),
            operation: EffectOp::Assign((true as usize).into()),
            prez: Lit::TRUE,
            source: None,
        });
    }
    for h in 0..instance.hangars.len() {
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin,
            transition_end: model.origin,
            mutex_end,
            state_var: is_empty(JigHolder::Hangar, h),
            operation: EffectOp::Assign((true as usize).into()),
            prez: Lit::TRUE,
            source: None,
        });
    }
}
