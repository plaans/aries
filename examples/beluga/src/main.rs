#![allow(dead_code)]

mod utils;

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

    states::initial_state(&instance, &mut model);
    //states::end_state(&instance, &mut model);

    let mut actions = Vec::new();

    for (b, beluga) in instance.flights.iter().enumerate() {

        let mut precedence : [VarCst; 2] = [(-1).into(); 2];
        for &j_in in beluga.incoming.iter() {

            let start : VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_beluga_to_rack(j_in.into(), b.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }

        let mut precedence : [VarCst; 2] = [VarCst {var : Var::ZERO, shift : -1} ; 2];
        for &j_out in beluga.outgoing.iter() {
            let start : VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_rack_to_beluga(j_out.into(), b.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }
    }

    

    for (pl, prod_line) in instance.production_lines.iter().enumerate() {
        let mut precedence : [VarCst; 2] = [VarCst {var : Var::ZERO, shift : -1} ; 2];
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
            ],
            jigs: vec![
                instance::Jig {
                    name: String::from("jig001"),
                    jig_type: 0,
                    empty: true,
                },
                instance::Jig {
                    name: String::from("jig002"),
                    jig_type: 0,
                    empty: true,
                },
            ],
            trailers: vec![
                instance::Trailer {name : String::from("beluga_trailer_1"), side : Side::Beluga},
                instance::Trailer {name : String::from("factory_trailer_1"), side : Side::Factory}
            ],
            hangars: vec!["hangar0".to_string()],
            racks: vec![instance::Rack {
                name : "rack0".to_string(),
                size : 32,
                jigs : vec![],
            }],
            production_lines: vec![instance::ProductionLine {
                name : "pl0".to_string(),
                schedule : vec![0, 1]
            }],
            flights: vec![
                instance::Flight {
                    name : String::from("beluga0"),
                    incoming : vec![0, 1],
                    outgoing : vec![0, 1],
                }
            ],
        }
    }

fn main() {
    /* 
    let file_path = "../instances/problem_s1_j13_r2_oc00_f3.json";
    let instance = instance::Instance::build(file_path).unwrap_or_else(|err| {
        println!("Application error: {err}");
        process::exit(1);
    });
    println!("{:#?}", instance.size_of_jig(0));*/
    let plan = solve(instance_test_1());
    if let Some(plan) = plan {
        println!("Found plan:");
        for op in plan {
            println!(" - {op:?}");
        }
    } else {
        println!("No plan...")
    }
}
