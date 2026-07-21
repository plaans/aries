#![allow(unused)]

use std::iter;

use aries_solver::prelude::*;
use aries_solver::{core::state::Evaluable, lang::ModelView};
use aries_timelines::explain::ExplainableSolver;
use aries_timelines::{constraints::HasValueAt, symbols::ObjectEncoding, *};
use itertools::Itertools;

struct Problem {
    /// number of trucks available
    num_trucks: usize,
    /// Location to be visited by at least one truck
    visits: Vec<Loc>,
    /// initial and final location of all truck
    depot: Loc,
}

impl Problem {
    /// First and last location (used as bounds on the variables)
    fn locations(&self) -> (Loc, Loc) {
        (
            *self.visits.iter().chain(iter::once(&self.depot)).min().unwrap(),
            *self.visits.iter().chain(iter::once(&self.depot)).max().unwrap(),
        )
    }
    /// First and last trucks (used as bounds on variables)
    fn trucks(&self) -> (IntCst, IntCst) {
        assert!(self.num_trucks > 0);
        (1, self.num_trucks as IntCst)
    }
}

/// A location is encoded as an integer constant
type Loc = IntCst;

fn solve_routing(pb: &Problem) -> Option<Vec<Op>> {
    let objects = ObjectEncoding::empty();
    let fluents = FluentsEncoding::empty();
    let mut model = aries_timelines::Sched::new(1, objects, fluents);

    let (first_truck, last_truck) = pb.trucks();

    let mut actions = Vec::new();

    for t in first_truck..=last_truck {
        // Initial state (at origin):
        // at the temporal origin, the truck is at the depot
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin, // at origin
            transition_end: model.origin,
            mutex_end,
            state_var: truck_loc(t),                      // the location of this truck
            operation: EffectOp::Assign(pb.depot.into()), // takes the value `depot`
            prez: Lit::TRUE,
            source: None,
        });

        // Goal state (at horizon):
        // the truck must be at the depot
        // Placing a condition like this forces the solver to find an effect that establishes th required value
        model.add_constraint(HasValueAt {
            state_var: truck_loc(t),  // the state variable denoting the location of this truck
            value: pb.depot.into(),   // must have the value `depot`
            timepoint: model.horizon, // at the end of the plan
            prez: Lit::TRUE,          // this must always be true
            source: None,             // not tied to any task
        });

        // each truck has an action that allows returning to the depot
        // This action is optional (and indeed not needed if the truck never leaves the depot)
        actions.push(add_move_to_action(Some(t), pb.depot, &mut model, pb))
    }

    for &loc in &pb.visits {
        // for each place to visit, add an action where a truck (unspecified which one) can go there
        let visit_loc_action = add_move_to_action(None, loc, &mut model, pb);

        // in a TSP like problem, the visit is mandatory, so we force this action to be present
        model.add_constraint(visit_loc_action.presence);
        actions.push(visit_loc_action);
    }
    let mut solver: ExplainableSolver<()> = aries_timelines::explain::ExplainableSolver::new(&model, |_| None);
    if let Some(solution) = solver.find_optimal(model.makespan.into(), |_| {}, vec![]) {
        let mut operations: Vec<Op> = actions.iter().filter_map(|act| act.evaluate(&solution)).collect_vec();
        operations.sort_by_key(|op| (op.truck, op.start));
        Some(operations)
    } else {
        // no plan found
        None
    }
}

/// Returns the state variable representing the location of a truck
fn truck_loc(truck: impl Into<LinTerm>) -> StateVar {
    StateVar {
        fluent: "loc".to_string(),
        args: vec![truck.into()],
    }
}

/// Adds an action that allows moving either a specific truck or any truck from its current location to a specified location
fn add_move_to_action(truck: Option<IntCst>, to: IntCst, model: &mut Sched, pb: &Problem) -> MoveTo {
    // literal encoding whether the action is present in the solution
    let presence = model.new_bool_var();

    // variables encoding the start and end time of the action
    let start: VarCst = model.new_opt_timepoint(presence);
    let end: VarCst = start + 1; // assumes a duration of 1

    // variable denoting which truck is moved by this action
    let (first_truck, last_truck) = pb.trucks();
    let truck: VarCst = if let Some(truck) = truck {
        truck.into() // a specific truck is specified for this action
    } else {
        // no truck specified, make it a parameter by creating a variable
        model.new_optional_var(first_truck, last_truck, presence).into()
    };

    // create a variable capturing the initial location of the truck
    let (first_loc, last_loc) = pb.locations();
    let from = model.new_optional_var(first_loc, last_loc, presence);

    // records a new task for this action
    // The tasks is necessary to determine things like the makespan of the plan (max end time of all present tasks)
    // We use the returned task id to attach the conditions and effect to it.
    let task_id = model.add_task(Task {
        name: "move".to_string(),
        start,
        end,
        presence,
        args: vec![
            (truck.into(), "truck".into()),
            (from.into(), "location".into()),
            (to.into(), "location".into()),
        ],
    });

    // effect that updates the truck location
    // [start, end] loc(truck) <- to
    let mutex_end = model.new_opt_timepoint(presence);
    model.add_effect(Effect {
        transition_start: start,     // the state variable looses its previous value at the action start
        transition_end: end,         // and is assigned its new value at the action end
        mutex_end,                   //
        state_var: truck_loc(truck), // modifies the location of the truck
        operation: EffectOp::Assign(to.into()), // assign it the destination `to`
        prez: presence,              // effect is only present when the action is
        source: Some(task_id),       // effect is introduced by this action
    });

    // condition that binds the truck location at the action start
    // [start] loc(truck) == from
    model.add_constraint(HasValueAt {
        state_var: truck_loc(truck), // state variable denoting the location of the truck
        value: from.into(),          // must have the value `from` (action parameter)
        timepoint: start,            // at the action start
        prez: presence,              // only needs to hold when the action is present
        source: Some(task_id),       // condition introduced as part of the current action
    });

    // returns a representation of the action, to allow accessing its parameters
    // and reconstruct it in the solution
    MoveTo {
        presence,
        start,
        truck,
        from,
        to,
        task_id,
    }
}

struct MoveTo {
    presence: Lit,
    start: VarCst,
    truck: VarCst,
    from: Var,
    to: IntCst,
    task_id: TaskId,
}

impl Evaluable for MoveTo {
    type Value = Op;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        if !solution.entails(self.presence) {
            // action is absent
            return None;
        }
        Some(Op {
            start: solution.eval(self.start).unwrap(),
            truck: solution.eval(self.truck).unwrap(),
            op: format!("move({}, {})", solution.eval(self.from).unwrap(), self.to),
        })
    }
}

/// Represents an actual operation in the plan
#[derive(Debug)]
struct Op {
    start: IntCst,
    truck: IntCst,
    op: String,
}

fn main() {
    let plan = solve_routing(&Problem {
        num_trucks: 2,
        visits: vec![1, 2, 3],
        depot: 0,
    });
    if let Some(plan) = plan {
        println!("Found plan:");
        for op in plan {
            println!(" - {op:?}");
        }
    } else {
        println!("No plan...")
    }
}
