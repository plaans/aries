use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use anyhow::{anyhow, Error};

use aries::core::Lit;
use aries::model::extensions::{SavedAssignment, Shaped};
use aries::model::{Label, Model};
use aries::solver::parallel::ParSolver;
use aries::solver::{Exit, Solver, UnsatCore};
use aries_explainability::musmcs_enumeration::marco::Marco;
use aries_explainability::musmcs_enumeration::marco::subsolvers::SubsetSolverImpl;
use aries_explainability::musmcs_enumeration::{MusMcsEnumerationConfig, MusMcsEnumerationResult};
use aries_grpc_server::serialize::serialize_plan;
use aries_planners::encode::{encode, EncodedProblem};
use aries_planners::encoding::Encoding;
use aries_planners::solver::{self, format_plan, Strat};
use aries_planners::solver::SolverResult;

use aries_planning::chronicles::{self, VarLabel};

use aries_grpc_server::chronicles::problem_to_chronicles;

use aries_planning::chronicles::FiniteProblem;
use clap::Parser;
use itertools::Itertools;
use prost::Message;
use unified_planning as up;

use beluga_study::io;

fn make_encoded_beluga_problem(
    mut base_problem: chronicles::Problem,
) -> Result<(EncodedProblem, Arc<FiniteProblem>), Error> {

    // Printer::print_problem(&base_problem);
    aries_planning::chronicles::preprocessing::preprocess(&mut base_problem);
    // Printer::print_problem(&base_problem);

    let metadata = Arc::new(aries_planning::chronicles::analysis::analyse(&base_problem));

    let finite_problem = FiniteProblem {
        model: base_problem.context.model.clone(),
        origin: base_problem.context.origin(),
        horizon: base_problem.context.horizon(),
        makespan_ub: base_problem.context.makespan_ub(),
        chronicles: base_problem.chronicles.clone(),
        meta: metadata.clone(),
    };
    
    // NOTE: the beluga problem is encoded as an optional scheduling problem, without a task network nor any templates to instantiate
    debug_assert!(base_problem.chronicles.iter().all(|c| c.chronicle.subtasks.is_empty()));
    debug_assert!(base_problem.templates.is_empty());

    let finite_problem = Arc::new(finite_problem);

    match encode(&finite_problem, None) {
        Ok(encoded_problem) => Ok((encoded_problem, finite_problem)),
        Err(conflict) => Err(anyhow!(format!("Encountered conflict {:?} when encoding/processing finite problem", conflict)))
    }
}

pub fn get_property_ids_to_varlabels_map(base_problem: &chronicles::Problem) -> (HashMap<String, VarLabel>, HashMap<VarLabel, String>) {
    let properties_varlabels = base_problem
        .context
        .model
        .shape
        .labels
        .all_labels()
        .into_iter()
        .filter_map(|lbl| match &lbl.1 {
            chronicles::VarType::Parameter(s) if s.starts_with("prop_") => {
                // NOTE: assumes the format "prop_<prop-id-without-underscores>_....."
                Some((s.split_once("_").unwrap().1.to_string(), lbl))
            },
            _ => None,
        })
        .collect::<HashMap<_,_>>();
    let properties_varlabels_rev = properties_varlabels
        .iter()
        .map(|(prop_id, lbl)| (lbl.clone(), prop_id.clone()))
        .collect::<HashMap<_,_>>();
    (properties_varlabels, properties_varlabels_rev)
}

/// WARNING: this solving procedure solves only for a fixed amount of allowed swaps.
pub fn solve_finite_beluga_with_given_properties(
    encoded_problem: EncodedProblem,
    finite_problem: Arc<FiniteProblem>,
    deadline_to_solve: Option<f64>,
    properties_lits: Vec<Lit>,
) -> SolverResult<Arc<SavedAssignment>> {

    let encoding = Arc::new(encoded_problem.encoding);

    let mut model_w_enforced_properties = encoded_problem.model;
    model_w_enforced_properties.enforce_all(properties_lits, []);

    let start = std::time::Instant::now();
    let deadline = deadline_to_solve.map(|val| start + std::time::Duration::from_secs_f64(val));
    let strategies = &[
        Strat::ActivityBool,
        Strat::ActivityBoolLight,
        Strat::Causal,
    ];

    let stn_config = aries::reasoners::stn::theory::StnConfig {
        theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
        ..Default::default()
    };
    let mut solver = Box::new(aries::solver::Solver::new(model_w_enforced_properties));
    solver.reasoners.diff.config = stn_config;

    let mut solver = aries::solver::parallel::ParSolver::new(solver, strategies.len(), |id, s| {
        strategies[id].adapt_solver(s, finite_problem.clone(), encoding.clone())
    });

    let result = solver.solve(deadline);

    if let SolverResult::Sol(_) = result {
        solver.print_stats()
    }
    println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());
    result
}

pub fn enumerate_finite_beluga_property_muses_and_mcses(
    encoded_problem_model: Model<VarLabel>,
    encoded_problem_encoding: Encoding,
    finite_problem: Arc<FiniteProblem>,
    deadline_to_enumerate: Option<f64>,
    properties_lits: &HashMap<Lit, VarLabel>,
) -> MusMcsEnumerationResult {

    // let start = std::time::Instant::now();
    // let deadline = deadline_to_enumerate.map(|val| start + std::time::Duration::from_secs_f64(val));
    // let strategies = &[
    //     Strat::ActivityBool,
    //     Strat::ActivityBoolLight,
    //     Strat::Causal,
    // ];

    let properties_lits_cloned = properties_lits.clone();
    let properties_lits_cloned2 = properties_lits.clone();

    // let subset_solver_impl = Box::new(VerySimpleSubsetSolverImpl::new(encoded_problem_model, finite_problem.clone(), encoded_problem_encoding.into()));
    let subset_solver_impl = Box::new(SimpleSubsetSolverImpl::new(encoded_problem_model, finite_problem.clone(), encoded_problem_encoding.into()));
    // let mut subset_solver_impl = Box::new(SimpleNonWorking2SubsetSolverImpl::new(encoded_problem_model, finite_problem.clone(), encoded_problem_encoding.into()));

    let mut marco = Marco::with_reified_soft_constraints(
        subset_solver_impl,
        properties_lits.keys().copied().collect::<Vec<_>>(),
        MusMcsEnumerationConfig {
            return_muses: true,
            return_mcses: true,
            on_mus_found: Some(Box::new(
                move |mus: &BTreeSet<Lit>| {
                    let mus_str = mus
                        .iter()
                        .map(|l| properties_lits_cloned.get(l).unwrap())
                        .join(", \n");
                    let mus_str = format!(r#"{}"#, mus_str);
                    println!("propMUS: {{\n{mus_str}\n}}\n");
            })),
            on_mcs_found: Some(Box::new(
                move |mus: &BTreeSet<Lit>| {
                    let mcs_str = mus
                        .iter()
                        .map(|l| properties_lits_cloned2.get(l).unwrap())
                        .join(", \n");
                    let mcs_str = format!(r#"{}"#, mcs_str);
                    println!("propMCS: {{\n{mcs_str}\n}}\n");
            })),
        },
    );

    let marco_res = marco.run();

    println!("\n");
    println!("{marco_res:?}");

    println!("MUSes: \n");
    for mus in marco_res.muses.as_ref().unwrap() {
        let mus_str = mus
            .iter()
            .map(|l| properties_lits.get(l).unwrap())
            .join(", \n");
        let mus_str = format!(r#"{}"#, mus_str);
        println!("{{\n{mus_str}\n}}\n");
    }

    println!("MCSes: \n");
    for mcs in marco_res.mcses.as_ref().unwrap() {
        let mcs_str = mcs
            .iter()
            .map(|l| properties_lits.get(l).unwrap())
            .join(", \n");
        let mcs_str = format!(r#"{}"#, mcs_str);
        println!("{{\n{mcs_str}\n}}\n");
    }

    marco_res
}

struct VerySimpleSubsetSolverImpl<Lbl: Label> {
    solver: Solver<Lbl>,
    finite_problem: Arc<FiniteProblem>,
    encoding: Arc<Encoding>,
    strats: [Strat; 3]
}
impl<Lbl: Label> VerySimpleSubsetSolverImpl<Lbl> {
    pub fn new(model: Model<Lbl>, finite_problem: Arc<FiniteProblem>, encoding: Arc<Encoding> ) -> Self {
        let mut solver = Solver::new(model);
        solver.reasoners.diff.config = aries::reasoners::stn::theory::StnConfig {
            theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
            ..Default::default()
        };
        let strats = [
            solver::Strat::ActivityBool,
            solver::Strat::ActivityBoolLight,
            solver::Strat::Causal,
            // solver::Strat::Forward, // causes BUG !!
        ];    
        Self { solver, finite_problem, encoding, strats }
    }
}
impl SubsetSolverImpl<VarLabel> for VerySimpleSubsetSolverImpl<VarLabel> {
    fn get_model(&mut self) -> &mut Model<VarLabel> {
        &mut self.solver.model
    }
    fn find_unsat_core(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit> {
        let mut base_solver = Box::new(self.solver.clone());
        base_solver.enforce_all(subset.clone(), []);

        let mut par_solver = ParSolver::new(
            base_solver,
            self.strats.len(),
            |id, s| {
                self.strats[id].adapt_solver(s, self.finite_problem.clone(), self.encoding.clone());
            },
        );
        // let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(300.0));
        let deadline = None;
        let sss = std::time::Instant::now();
        let res = match par_solver.solve(deadline) {
            aries::solver::parallel::SolverResult::Sol(_) => { 
                let t = sss.elapsed(); 
                println!("{t:?}"); 
                Ok(Ok(()))
            },
            _ => {
                let mut unsat_core: aries::core::state::Explanation = aries::core::state::Explanation::new();
                unsat_core.extend(subset.clone());
                println!("unsat core: {unsat_core:?}");

                self.solver.enforce(aries::model::lang::expr::or(unsat_core.literals().iter().map(|&l| !l).collect::<Vec<_>>().into_boxed_slice()), []);

                Ok(Err(unsat_core))
            },
        };
        res
    }
}

struct SimpleSubsetSolverImpl<Lbl: Label> {
    solver: Solver<Lbl>,
    finite_problem: Arc<FiniteProblem>,
    encoding: Arc<Encoding>,
    strats: [Strat; 3]
}
impl<Lbl: Label> SimpleSubsetSolverImpl<Lbl> {
    pub fn new(model: Model<Lbl>, finite_problem: Arc<FiniteProblem>, encoding: Arc<Encoding>) -> Self {
        let mut solver = Solver::new(model);
        solver.reasoners.diff.config = aries::reasoners::stn::theory::StnConfig {
            theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
            ..Default::default()
        };
        let strats = [
            solver::Strat::ActivityBool,
            solver::Strat::ActivityBoolLight,
            solver::Strat::Causal,
        ];    
        Self { solver, finite_problem, encoding, strats }
    }
}
impl SubsetSolverImpl<VarLabel> for SimpleSubsetSolverImpl<VarLabel> {
    fn get_model(&mut self) -> &mut Model<VarLabel> {
        &mut self.solver.model
    }
    fn find_unsat_core(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit> {

        let mut par_solver = ParSolver::new(
            Box::new(self.solver.clone()),
            self.strats.len(),
            |id, s| {
                self.strats[id].adapt_solver(s, self.finite_problem.clone(), self.encoding.clone());
            },
        );
        // let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(300.0));
        let deadline = None;
        let sss = std::time::Instant::now();
        match par_solver.solve_with_assumptions(subset.iter().copied().collect(), deadline) {
            aries::solver::parallel::SolverResult::Sol(_) => { 
                let t = sss.elapsed(); 
                println!("{t:?}"); 
                Ok(Ok(()))
            },
            aries::solver::parallel::SolverResult::Unsat(unsat_core) => {
                let lendiff = subset.len() - unsat_core.as_ref().unwrap().literals().len();
                println!("{unsat_core:?} | lendiff:{lendiff:?}");
                self.solver.enforce(aries::model::lang::expr::or(unsat_core.as_ref().unwrap().literals().iter().map(|&l| !l).collect::<Vec<_>>().into_boxed_slice()), []);
                Ok(Err(unsat_core.unwrap()))
            },
            aries::solver::parallel::SolverResult::Timeout(_) => {
                let mut unsat_core: aries::core::state::Explanation = aries::core::state::Explanation::new();
                unsat_core.extend(subset.iter().copied());
                self.solver.enforce(aries::model::lang::expr::or(unsat_core.literals().iter().map(|&l| !l).collect::<Vec<_>>().into_boxed_slice()), []);
                Ok(Err(unsat_core))
            },
        }
    }
}

pub fn main() -> Result<(), Error> {

    let args = io::Cli::parse();
    
    match args.command {
        io::Command::Solve(solve_args) => {
            let problem_file_path = solve_args.problem_file_path;
            let problem = std::fs::read(problem_file_path)?;
            let problem = up::Problem::decode(problem.as_slice())?;
            let problem = Arc::new(problem);

            let base_problem = problem_to_chronicles(&problem)?;
            // let mut base_problem = problem_to_chronicles(&problem)?;
            // base_problem.context.model.enforce_all::<Lit>(base_problem.chronicles.iter().map(|c| &c.chronicle.presence).copied(), []);

            let (properties_varlabels, _) = get_property_ids_to_varlabels_map(&base_problem);

            let (encoded_problem, finite_problem) = make_encoded_beluga_problem(base_problem)?;

            let properties_lits = properties_varlabels
                .iter()
                .map(|(_, lbl)| encoded_problem.model.get_var(&lbl).unwrap().geq(1))
                .collect::<Vec<_>>();

            // Will solve for all properties being enforced
            let result = solve_finite_beluga_with_given_properties(
                encoded_problem,
                finite_problem.clone(),
                None,
                properties_lits,
            );

            let (plan_str, plan) = match result {
                aries::solver::parallel::SolverResult::Sol(plan) => (
                    Some(format_plan(&finite_problem, &plan, false)?),
                    Some(serialize_plan(&problem, &finite_problem, &plan)?),
                ),
                aries::solver::parallel::SolverResult::Unsat(_) => (None, None),
                aries::solver::parallel::SolverResult::Timeout(_) => (None, None),
            };

            println!("Result plan: ");
            if let Some(plan_str) = plan_str {
                println!("{plan_str}");
            } else {
                println!("None");
            }

            return Ok(())
        },
        io::Command::Explain(explain_args) => {

            let problem_file_path = explain_args.problem_file_path;
            let problem = std::fs::read(problem_file_path)?;
            let problem = up::Problem::decode(problem.as_slice())?;
            let problem = Arc::new(problem);

            let base_problem = problem_to_chronicles(&problem)?;

            let (properties_varlabels, properties_var_labels_rev) = get_property_ids_to_varlabels_map(&base_problem);

            // let (encoded_problem, finite_problem) = make_encoded_beluga_problem(base_problem)?;
            let (encoded_problem, finite_problem) = make_encoded_beluga_problem(base_problem.clone())?;

            let properties_lits = properties_varlabels
                .iter()
                .map(|(_, lbl)| {
                    let var = encoded_problem.model.get_var(&lbl).unwrap();
                    (var.geq(1), lbl.clone())   
                })
                .collect::<HashMap<_,_>>();

            let encoded_problem_encoding = encoded_problem.encoding.clone();
            let mut encoded_problem_model = encoded_problem.model.clone();

            let result = enumerate_finite_beluga_property_muses_and_mcses(
                encoded_problem_model,
                encoded_problem_encoding,
                // finite_problem,
                finite_problem.clone(),
                None,
                &properties_lits,
            );

            let results_file_path = explain_args.results_file_path;

            let prop_ids_muses = result
                .muses
                .as_ref()
                .unwrap_or(&vec![])
                .iter().map(|mus| {
                    mus.iter().map(|l| format!("{}", properties_var_labels_rev.get(properties_lits.get(l).unwrap()).unwrap())).collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();

            let prop_ids_mcses = result
                .mcses
                .as_ref()
                .unwrap_or(&vec![])
                .iter().map(|mcs| {
                    mcs.iter().map(|l| format!("{}", properties_var_labels_rev.get(properties_lits.get(l).unwrap()).unwrap())).collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
        
            let _ = io::write_mus_mcs_enumeration_result_to_file(
                results_file_path,
                result.complete,
                prop_ids_muses,
                prop_ids_mcses,
            )?;
        },
    };
    Ok(())
}