#![allow(dead_code)]

mod utils;

use std::process;

use aries_timelines::symbols::ObjectEncoding;
use utils::*;
use aries_solver::prelude::*;
use aries_solver::{core::state::Evaluable};
use aries_timelines::explain::ExplainableSolver;
use itertools::Itertools;
use utils::actions::*;



fn solve(instance : instance::Instance) -> Option<Vec<Op>> {
    let objects = ObjectEncoding::empty();
    let mut model = aries_timelines::Sched::new(1, objects);

    states::set_initial_state(&mut model, &instance);
    states::set_end_state(&mut model, &instance);

  
    let mut actions : Vec<Action> = Vec::new();
/* 
    let mut locations : Vec<(Var, Var)> = Vec::new();

    let param = ActParam {
        start : 2.into(),
        end : 2.into(),
        presence : Lit::TRUE,
        source : None
    };

    let start : VarCst = model.new_opt_timepoint(Lit::TRUE);

    add_beluga_to_rack(0.into(), 0.into(), start, &mut model, &mut actions, &instance);
    let start2 : VarCst = model.new_opt_timepoint(Lit::TRUE);
    add_beluga_to_rack(1.into(), 0.into(), start2, &mut model, &mut actions, &instance);

    model.add_constraint(lt(start, start2));
    let mut solver: ExplainableSolver<()> = aries_timelines::explain::ExplainableSolver::new(&model, |_| None);

    if let Some(solution) = solver.check_satisfiability() {
        let sol : Vec<(i32, i32)> = locations.iter().map(|(holder, num)| (solution.eval(*holder).unwrap(), solution.eval(*num).unwrap())).collect();
        println!("{:#?}", sol);
        //println!("t = {:?}", solution.eval(t).unwrap());
        let mut operations: Vec<Op> = actions.iter().filter_map(|act| act.evaluate(&solution)).collect_vec();
        operations.sort_by_key(|op| op.start);
        Some(operations)
    } else {
        // no plan found
        println!("{:#?}", actions);
        None
    } */


    for (b, beluga) in instance.flights.iter().enumerate() {
    // let b = 0;
    // let beluga = & instance.flights[0];

        let mut precedence : [VarCst; 2] = [(-1).into(); 2];
        for &j_in in beluga.incoming.iter() {

            let start : VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_beluga_to_rack(j_in.into(), b.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }

        let start_switch : VarCst = model.new_opt_timepoint(Lit::TRUE);
        model.add_constraint(lt(precedence[1], start_switch));

        let mut precedence : [VarCst; 2] = [VarCst {var : Var::ZERO, shift : -1} ; 2];
        for &j_out in beluga.outgoing.iter() {

            let start : VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_rack_to_beluga(j_out, b.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }
        
        add_switch_to_next_beluga(b, start_switch, &mut model, &mut actions, &instance);
        model.add_constraint(lt(precedence[1], start_switch));
    }


    for (pl, prod_line) in instance.production_lines.iter().enumerate() {
        let mut precedence : [VarCst; 2] = [(-1).into() ; 2];
        for &j in prod_line.schedule.iter() {
            let start : VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_send_to_prod(j.into(), pl.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }
    }
    //Les actions ne peuvent pas se chevaucher
    let starts  : Vec<VarCst> = actions.iter().map(|a| a.start).collect();
    //model.add_constraint(all_different(starts));
    

    let mut solver: ExplainableSolver<()> = aries_timelines::explain::ExplainableSolver::new(&model, |_| None);

    if let Some(solution) = solver.check_satisfiability() {
        let mut operations: Vec<Op> = actions.iter().filter_map(|act| act.evaluate(&solution)).collect_vec();
        operations.sort_by_key(|op| op.start);
        Some(operations)
    } else {
        // no plan found
        println!("{:#?}", actions);
        None
    }

}

fn instance_test_1() -> instance::Instance {
        instance::Instance {
            jig_types: vec![
                instance::JigType {
                    name: String::from("typeA"),
                    size_empty: 4,
                    size_loaded: 6,
                },
                instance::JigType {
                    name: String::from("typeB"),
                    size_empty: 8,
                    size_loaded: 11,
                },
            ],
            jigs: vec![
                instance::Jig {
                    name: String::from("jig001"),
                    jig_type: 0,
                    empty: false,
                },
                instance::Jig {
                    name: String::from("jig002"),
                    jig_type: 1,
                    empty: false,
                },
                instance::Jig {
                    name: String::from("jig003"),
                    jig_type: 0,
                    empty: false,
                },
            ],
            trailers_beluga: vec![
                instance::Trailer {name : String::from("beluga_trailer_1"), side : Side::Beluga},
            ],
            trailers_factory: vec![
                instance::Trailer {name : String::from("beluga_factory_1"), side : Side::Factory},
            ],
            hangars: vec!["hangar0".to_string()],
            racks: vec![instance::Rack {
                name : "rack00".to_string(),
                size : 26,
                jigs : vec![],
            },],
            production_lines: vec![
                instance::ProductionLine {
                    name : "pl1".to_string(),
                    schedule : vec![0]
                }
            ],
            flights: vec![
                instance::Flight {
                    name : String::from("beluga1"),
                    incoming : vec![0],
                    outgoing : vec![],
                },
                instance::Flight {
                    name : String::from("beluga2"),
                    incoming : vec![1,2],
                    outgoing : vec![],
                },
            ],
        }
    }

fn main() {
    let file_path = "examples/beluga/instances/problem_s1_j4_r2_oc00_f3.json";
    let instance = instance::Instance::build(file_path).unwrap_or_else(|err| {
        println!("Application error: {err}");
        process::exit(1);
    });
    //println!("{:#?}", instance);
    let plan = solve(instance);
    if let Some(plan) = plan {
        println!("Found plan:");
        for op in plan {
            println!(" - {op:?}");
        }
    } else {
        println!("No plan...")
    }
}
