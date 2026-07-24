#![allow(dead_code)]

mod utils;

use std::process;

use aries_solver::core::state::Evaluable;
use aries_solver::lang::ModelView;
use aries_solver::prelude::*;
use aries_timelines::explain::ExplainableSolver;
use aries_timelines::symbols::ObjectEncoding;
use itertools::Itertools;
use std::time::{Duration, Instant};
use utils::actions::*;
use utils::*;

fn solve(instance: &instance::Instance, n_swaps_beluga: u32, n_swaps_factory: u32) -> Option<Vec<Op>> {
    let objects = ObjectEncoding::empty();
    let mut model = aries_timelines::Sched::new(1, objects);

    states::set_initial_state(&mut model, &instance);

    let mut actions: Vec<Action> = Vec::new();

    //Flights
    for (b, beluga) in instance.flights.iter().enumerate() {
        let mut precedence: [VarCst; 2] = [(-1).into(); 2];
        for &j_in in beluga.incoming.iter() {
            let start: VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_beluga_to_rack(j_in.into(), b.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }

        let mut precedence: [VarCst; 2] = [(-1).into(); 2];
        for &j_out in beluga.outgoing.iter() {
            let start: VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_rack_to_beluga(j_out, b.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }

        let start_switch: VarCst = model.new_opt_timepoint(Lit::TRUE);
        add_switch_to_next_beluga(b, start_switch, &mut model, &mut actions, &instance);
    }

    //Production Lines
    for (pl, prod_line) in instance.production_lines.iter().enumerate() {
        let mut precedence: [VarCst; 2] = [(-1).into(); 2];
        for &j in prod_line.schedule.iter() {
            let start: VarCst = model.new_opt_timepoint(Lit::TRUE);
            add_send_to_prod(j.into(), pl.into(), start, &mut model, &mut actions, &instance);

            //precedence
            precedence[0] = precedence[1];
            precedence[1] = start;
            model.add_constraint(lt(precedence[0], precedence[1]));
        }
    }

    //Swapping
    for _i in 0..n_swaps_beluga {
        add_swap_racks(Side::Beluga, Lit::TRUE, &mut model, &mut actions, &instance);
    }
    for _i in 0..n_swaps_factory {
        add_swap_racks(Side::Factory, Lit::TRUE, &mut model, &mut actions, &instance);
    }

    //Solving
    let mut solver: ExplainableSolver<()> = aries_timelines::explain::ExplainableSolver::new(&model, |_| None);
    let solution: Option<Solution> = solver.check_satisfiability();
    match solution {
        Some(solution) => {
            let mut operations: Vec<Op> = actions.iter().filter_map(|act| act.evaluate(&solution)).collect_vec();
            operations.sort_by_key(|op| op.start);
            Some(operations)
        }
        None => {
            // no plan found
            None
        }
    }
}

fn find_optimal(instance: instance::Instance) {
    let max_n_swaps = 5;
    for n_swaps in 0..=max_n_swaps {
        for i in 0..=n_swaps {
            let now = Instant::now();
            let n_swaps_beluga = i;
            let n_swaps_factory = n_swaps - n_swaps_beluga;
            println!("\nTrying to solve with {n_swaps_beluga} swaps_beluga and {n_swaps_factory} swaps_factory");
            let sol = solve(&instance, n_swaps_beluga, n_swaps_factory);
            print_runtime(now.elapsed());
            if let Some(ops) = &sol {
                println!("Found solution with {n_swaps_beluga} swaps_beluga and {n_swaps_factory} swaps_factory");
                for op in ops {
                    println!(" - {op:?}");
                }
                return;
            }
        }
    }
    println!("No solution found in {max_n_swaps} total max swaps...");
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
                jig_type: 0,
                empty: true,
            },
            instance::Jig {
                name: String::from("jig003"),
                jig_type: 1,
                empty: false,
            },
            instance::Jig {
                name: String::from("jig004"),
                jig_type: 1,
                empty: false,
            },
        ],
        trailers_beluga: vec![instance::Trailer {
            name: String::from("beluga_trailer_1"),
            side: Side::Beluga,
        }],
        trailers_factory: vec![instance::Trailer {
            name: String::from("beluga_factory_1"),
            side: Side::Factory,
        }],
        hangars: vec!["hangar0".to_string()],
        racks: vec![
            instance::Rack {
                name: "rack00".to_string(),
                size: 17,
                jigs: vec![0],
            },
            instance::Rack {
                name: "rack00".to_string(),
                size: 14,
                jigs: vec![1],
            },
        ],
        production_lines: vec![instance::ProductionLine {
            name: "pl0".to_string(),
            schedule: vec![2, 3],
        }],
        flights: vec![
            instance::Flight {
                name: String::from("beluga1"),
                incoming: vec![2, 3],
                outgoing: vec![],
            },
            instance::Flight {
                name: String::from("beluga1"),
                incoming: vec![],
                outgoing: vec![1, 0],
            },
        ],
    }
}

fn main() {
    let now = Instant::now();

    let file_path = "examples/beluga/instances/problem_s1_j6_r2_oc00_f3.json";
    let instance = instance::Instance::build(file_path).unwrap_or_else(|err| {
        println!("Application error: {err}");
        process::exit(1);
    });

    find_optimal(instance_test_1());
    print!("Total : ");
    print_runtime(now.elapsed());
}

fn print_runtime(time: Duration) {
    let millis: u16 = (time.as_millis() % 1000) as u16;
    let secs: u8 = (time.as_secs() % 60) as u8;
    let min: u64 = time.as_secs() / 60;
    println!("Executed in {}min {}s {}ms ({}ms)", min, secs, millis, time.as_millis());
}
