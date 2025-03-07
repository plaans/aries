use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, ensure, Context, Error};

use aries::core::INT_CST_MAX;
use aries::model::extensions::SavedAssignment;
use aries_explainability::explain::why::unsat::QwhyUnsat;
use aries_explainability::explain::{ModelAndVocab, Query, Question, Situation};
use aries_grpc_server::serialize::{engine, serialize_plan};
use aries_planners::encode::{encode, populate_with_task_network};
use aries_planners::solver::{self, format_plan};
use aries_planners::solver::SolverResult;

use aries_planning::chronicles::analysis::hierarchy::hierarchical_is_non_recursive;
use aries_planning::chronicles::{Ctx, VarLabel};

use aries_grpc_server::chronicles::{beluga_problem_to_chronicles, problem_to_chronicles};

use aries_planning::chronicles::FiniteProblem;
use prost::Message;
use unified_planning as up;

use beluga_study::explanation;
use beluga_study::io;

/// Solves the given problem, giving any intermediate solution to the callback.
/// NOTE: This function is CPU-Bound and should not be used in an async context
fn solve_beluga(
    problem: Arc<up::Problem>,
    on_new_sol: impl Fn(up::Plan) + Clone,
//    conf: Arc<SolverConfiguration>,
) -> Result<up::PlanGenerationResult, Error> {

    let (
        subtask_id_to_ch_instance_index_map,
        constraint_index_to_ch_instance_index_map,
        mut base_problem,
    ) = beluga_problem_to_chronicles(&problem)
        .with_context(|| format!("In problem {}/{}", &problem.domain_name, &problem.problem_name))?;

//    for (_, subtask_ch_instance_index) in subtask_id_to_ch_instance_index_map {
//        base_problem.context.model.enforce(base_problem.chronicles[subtask_ch_instance_index].chronicle.presence, [])
//    }
//    for (_, constraint_ch_instance_index) in constraint_index_to_ch_instance_index_map {
//        base_problem.context.model.enforce(base_problem.chronicles[constraint_ch_instance_index].chronicle.presence, [])
//    }

    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(90.0));

    let strategies = vec![]; // empty: will use default strategies

    let htn_mode = problem.hierarchy.is_some();

    let bounded = htn_mode && hierarchical_is_non_recursive(&base_problem) || base_problem.templates.is_empty();

    let optimal = false;

    ensure!(problem.metrics.is_empty(), "No metrics in beluga problem"); // FIXME: really ?
    let metric = None;
    
    let max_depth = 1;
    let min_depth = if bounded {
        max_depth // non recursive htn: bounded size, go directly to max
    } else {
        0
    };

    // callback that will be invoked each time an intermediate solution is found
    let on_new_solution = |pb: &FiniteProblem, ass: Arc<SavedAssignment>| {
        let plan = serialize_plan(&problem, pb, &ass);
        match plan {
            Ok(plan) => on_new_sol(plan),
            Err(err) => eprintln!("Error when serializing intermediate plan: {err}"),
        }
    };
    // run solver
    let result = solver::solve(
        base_problem,
        min_depth,
        max_depth,
        &strategies,
        metric,
        htn_mode,
        on_new_solution,
        deadline,
    )?;
    match result {
        SolverResult::Sol((finite_problem, plan)) => {
            println!(
                "************* SOLUTION FOUND **************\n\n{}",
                format_plan(&finite_problem, &plan, htn_mode)?
            );
            let status = if metric.is_some() && bounded {
                up::plan_generation_result::Status::SolvedOptimally
            } else {
                up::plan_generation_result::Status::SolvedSatisficing
            };
            let plan = serialize_plan(&problem, &finite_problem, &plan)?;
            Ok(up::PlanGenerationResult {
                status: status as i32,
                plan: Some(plan),
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(aries_grpc_server::serialize::engine()),
            })
        }
        SolverResult::Unsat => {
            println!("************* NO PLAN **************");
            Ok(up::PlanGenerationResult {
                status: up::plan_generation_result::Status::UnsolvableIncompletely as i32,
                plan: None,
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(engine()),
            })
        }
        SolverResult::Timeout(opt_plan) => {
            println!("************* TIMEOUT **************");
            let opt_plan = if let Some((finite_problem, plan)) = opt_plan {
                println!("\n{}", format_plan(&finite_problem, &plan, htn_mode)?);
                Some(serialize_plan(&problem, &finite_problem, &plan)?)
            } else {
                None
            };

            let status = if opt_plan.is_none() || optimal {
                up::plan_generation_result::Status::Timeout
            } else {
                up::plan_generation_result::Status::SolvedSatisficing
            };

            Ok(up::PlanGenerationResult {
                status: status as i32,
                plan: opt_plan,
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(engine()),
            })
        }
    }
}

pub fn main() -> Result<(), Error> {

    let (problem, q) = io::interpret_input("../test_pb_beluga_unsat1.upp".to_string(), "".to_string())?;
    let problem = Arc::new(problem);

    solve_beluga(problem, |_| {});
    return Ok(());

//    let (
//        subtask_id_to_ch_instance_index_map,
//        constraint_index_to_ch_instance_index_map,
//        mut base_problem,
//    ) = beluga_problem_to_chronicles(&problem)
//        .with_context(|| format!("In problem {}/{}", &problem.domain_name, &problem.problem_name))?;
//
//    // for (_, &constraint_ch_instance_index) in &constraint_index_to_ch_instance_index_map {
//    //     base_problem.context.model.enforce(base_problem.chronicles[constraint_ch_instance_index].chronicle.presence, [])
//    // }
//    // for (_, &subtask_ch_instance_index) in &subtask_id_to_ch_instance_index_map {
//    //     base_problem.context.model.enforce(base_problem.chronicles[subtask_ch_instance_index].chronicle.presence, [])
//    // }
//    
//    aries_planning::chronicles::preprocessing::preprocess(&mut base_problem);
//    let metadata = Arc::new(aries_planning::chronicles::analysis::analyse(&base_problem));
//
//    let mut finite_problem = FiniteProblem {
//        model: base_problem.context.model.clone(),
//        origin: base_problem.context.origin(),
//        horizon: base_problem.context.horizon(),
//        makespan_ub: base_problem.context.makespan_ub(),
//        chronicles: base_problem.chronicles.clone(),
//        meta: metadata.clone(),
//    };
//    populate_with_task_network(&mut finite_problem, &base_problem, 0)?;
//    let finite_problem = Arc::new(finite_problem);
//
//    let encoded_problem = encode(&finite_problem, None);
//
//    let (
//        model,
//        solve_fn,
//    ): (Arc<aries::model::Model<VarLabel>>, Box<dyn Fn(&mut aries::solver::Solver<VarLabel>) -> bool>) = match encoded_problem {
//        Ok(epb) => {
//            let encoding = Arc::new(epb.encoding);
//            (
//                Arc::new(epb.model),
//                Box::new(move |solver| {  
//
////                    let strats: [solver::Strat; 4] = [
////                        solver::Strat::ActivityBool,
////                        solver::Strat::ActivityBoolLight,
////                        solver::Strat::Causal,
////                        solver::Strat::Forward,
////                    ];
////                    let mut solver = aries::solver::parallel::ParSolver::new(Box::new(solver.clone()), strats.len(), |id, s| {
////                        strats[id].adapt_solver(s, finite_problem.clone(), encoding.clone())
////                    });
////                    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(120.0));
////                    let result = solver.solve(deadline);
////                
////                    if let SolverResult::Sol(_) = result {
////                        solver.print_stats();
////                        true
////                    } else {
////                        false
////                    }
//
//                    solver::Strat::ActivityBool.adapt_solver(solver, finite_problem.clone(), encoding.clone());
//
//                    let start = Instant::now();
//                    let result = solver.solve();
//                    if result.is_ok_and(|a| a.is_some()) {
//                        solver.print_stats();
//                        println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());
//                        solver.reset_search();
//                        true
//                    } else {
//                        solver.reset_search();
//                        false
//                    }
//                }),
//            )
//        },
//        Err(_) => (Arc::new(base_problem.context.model), Box::new(|_| false)),
//    };
//
//        // let solver = Box::new(solver.clone());
//        // 
//        // let strats = &[
//        //     solver::Strat::ActivityBool,
//        //     solver::Strat::ActivityBoolLight,
//        //     solver::Strat::Causal,
//        //     solver::Strat::Forward,
//        // ];
//        // let mut solver = aries::solver::parallel::ParSolver::new(solver, strats.len(), |id, s| {
//        //     strats[id].adapt_solver(s, finite_problem.clone(), encoding.clone())
//        // });
//        // 
//        // let start = Instant::now();
//        // let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(120.0));
//        // let result = solver.solve(deadline);
//        // 
//        // if let SolverResult::Sol(_) = result {
//        //     solver.print_stats();
//        //     println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());
//        // }
//        // match result {
//        //     aries::solver::parallel::SolverResult::Sol(_) => true,
//        //     aries::solver::parallel::SolverResult::Unsat => false,
//        //     aries::solver::parallel::SolverResult::Timeout(_) => false,
//        // }
//
//    let mut qwhyunsat = QwhyUnsat::<VarLabel, _>::new(
//        ModelAndVocab::new(
//            model, //Arc::new(model),
//            vec![], //constraint_index_to_ch_instance_index_map.iter().map(|(_, &i)| base_problem.chronicles[i].chronicle.presence),
//        ),
//        Situation::new(),
//        Query::from_iter(
//            subtask_id_to_ch_instance_index_map.iter().map(|(_, &i)| base_problem.chronicles[i].chronicle.presence)
//        ),
//        solve_fn,
//    );
//
//
//    let presupp_res = qwhyunsat.check_presuppositions();
//    println!("{presupp_res:?}");
//
//
//    // let expl = qwhyunsat.try_answer().unwrap();
//    // let essences = expl.essences;
//    // println!("{essences:?}");
//
//
//    Ok(())
//}
//
//
////pub fn main2() -> Result<(), Error> {
////    println!("Hello World");
////    // let (problem, q) = io::interpret_input("../test_pb_beluga_sat1.upp".to_string(), "".to_string())?;
////    let (problem, q) = io::interpret_input("../test_pb_beluga_unsat1.upp".to_string(), "".to_string())?;
////    let problem = Arc::new(problem);
////
//////    let answer = solve_beluga(problem, |_| {} );
//////
//////    match answer {
//////        Ok(res) => {
//////            let plan = if res.plan.is_some() { "PLAN FOUND" } else { "NO PLAN..." };
//////            let status = match up::plan_generation_result::Status::try_from(res.status) {
//////                Ok(s) => s.as_str_name(),
//////                Err(_) => "???",
//////            };
//////            println!("{plan}   ({status})")
//////        }
//////        Err(e) => bail!(e),
//////    }
////    
////    let (
////        subtask_id_to_ch_instance_index_map,
////        constraint_index_to_ch_instance_index_map,
////        mut base_problem,
////    ) = beluga_problem_to_chronicles(&problem)
////        .with_context(|| format!("In problem {}/{}", &problem.domain_name, &problem.problem_name))?;
////
//////    for (subtask_id, subtask_ch_instance_index) in subtask_id_to_ch_instance_index_map {
//////        base_problem.context.model.enforce(base_problem.chronicles[subtask_ch_instance_index].chronicle.presence, [])
//////    }
//////    for (constraint_index, constraint_ch_instance_index) in constraint_index_to_ch_instance_index_map {
//////        base_problem.context.model.enforce(base_problem.chronicles[constraint_ch_instance_index].chronicle.presence, [])
//////    }
////
//////    let mut solver = aries::solver::Solver::new(base_problem.context.model);
//////    solver.enforce_all(subtask_id_to_ch_instance_index_map.into_iter().map(|(_, i)| base_problem.chronicles[i].chronicle.presence), []);
//////    solver.enforce_all(constraint_index_to_ch_instance_index_map.into_iter().map(|(_, i)| base_problem.chronicles[i].chronicle.presence), []);
//////    let res = solver.solve().unwrap();
//////    let a = res.is_some(); println!("{a}");
////
////
////
////
////
////    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(90.0));
////
////    let htn_mode = problem.hierarchy.is_some();
////
////    let bounded = htn_mode && hierarchical_is_non_recursive(&base_problem) || base_problem.templates.is_empty();
////
////    let optimal = false;
////
////    ensure!(problem.metrics.is_empty(), "No metrics in beluga problem"); // FIXME: really ?
////    let metric = None;
////    
////    let max_depth = 1;
////    let min_depth = if bounded {
////        max_depth // non recursive htn: bounded size, go directly to max
////    } else {
////        0
////    };
////
////
////    println!("===== Preprocessing ======");
////    aries_planning::chronicles::preprocessing::preprocess(&mut base_problem);
////    println!("==========================");
////
////    let metadata = Arc::new(aries_planning::chronicles::analysis::analyse(&base_problem));
////
////    let mut best_cost = aries::core::INT_CST_MAX + 1;
////
////    let start = std::time::Instant::now();
////    for depth in min_depth..=max_depth {
////        let mut pb = FiniteProblem {
////            model: base_problem.context.model.clone(),
////            origin: base_problem.context.origin(),
////            horizon: base_problem.context.horizon(),
////            makespan_ub: base_problem.context.makespan_ub(),
////            chronicles: base_problem.chronicles.clone(),
////            meta: metadata.clone(),
////        };
////        let depth_string = if depth == u32::MAX {
////            "∞".to_string()
////        } else {
////            depth.to_string()
////        };
////        println!("{depth_string} Solving with depth {depth_string}");
////        if htn_mode {
////            aries_planners::encode::populate_with_task_network(&mut pb, &base_problem, depth)?;
////        } else {
////            aries_planners::encode::populate_with_template_instances(&mut pb, &base_problem, |_| Some(depth))?;
////        }
////        let pb = Arc::new(pb);
////
////        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
////
////        //let result = solver::solve_finite_problem(
////        //    pb.clone(),
////        //    &vec![],
////        //    metric,
////        //    htn_mode,
////        //    |_| {},
////        //    deadline,
////        //    best_cost - 1,
////        //);
////
////        let Ok(aries_planners::encode::EncodedProblem {
////            model: mut encoded_pb_model,
////            objective: None,
////            encoding,
////        }) = aries_planners::encode::encode(&pb, metric)
////        else {
////            println!("unsat");
////            return Ok(())
////        };
////
//////        let stn_config = aries::reasoners::stn::theory::StnConfig {
//////            theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
//////            ..Default::default()
//////        };    
//////        let mut solver = Box::new(aries::solver::Solver::new(encoded_pb_model));
//////        solver.reasoners.diff.config = stn_config;
//////
//////        let r = solver.solve().unwrap().is_some();
//////        println!("{r}");
//////
//////        // println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());
//////        // match result {
//////        //     SolverResult::Unsat => { println!("unsat"); }
//////        //     SolverResult::Sol(_) => { println!("sat"); }
//////        //     _ => panic!(),
//////        // }
////
////        encoded_pb_model.enforce_all(constraint_index_to_ch_instance_index_map.iter().map(|(_, &i)| base_problem.chronicles[i].chronicle.presence), []);
////        let mut qwhyunsat = QwhyUnsat::new(
//////            ModelAndVocab::new(
//////                Arc::new(encoded_pb_model),
//////                constraint_index_to_ch_instance_index_map.iter().map(|(_, &i)| base_problem.chronicles[i].chronicle.presence),
//////            ),
////            ModelAndVocab::new(Arc::new(encoded_pb_model), []),
////            Situation::new(),
////            Query::from_iter(
////                subtask_id_to_ch_instance_index_map.iter().map(|(_, &i)| base_problem.chronicles[i].chronicle.presence)
////            ),
////        );
////    
////        // let mut m = qwhyunsat.model_and_vocab.model_with_enforced_vocab();
////        // m.enforce_all(qwhyunsat.query, []);
////        
////        let expl = qwhyunsat.try_answer().unwrap();
////        let essences = expl.essences;
////        println!("{essences:?}");
////
////    }
////
////
////
//////    let mut qwhyunsat = QwhyUnsat::new(
//////        ModelAndVocab::new(
//////            Arc::new(base_problem.context.model),
//////            constraint_index_to_ch_instance_index_map.iter().map(|(_, &i)| base_problem.chronicles[i].chronicle.presence),
//////        ),
//////        Situation::new(),
//////        Query::from_iter(
//////            subtask_id_to_ch_instance_index_map.iter().map(|(_, &i)| base_problem.chronicles[i].chronicle.presence)
//////        ),
//////    );
//////
//////    let mut m = qwhyunsat.model_and_vocab.model_with_enforced_vocab();
//////    m.enforce_all(qwhyunsat.query, []);
//////    
//////
////////    let expl = qwhyunsat.try_answer().unwrap();
////////    let essences = expl.essences;
////////    println!("{essences:?}");
////    
////    Ok(())
}