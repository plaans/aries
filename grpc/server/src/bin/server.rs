use anyhow::{bail, ensure, Error};
use aries_grpc_server::chronicles::problem_to_chronicles;
use aries_grpc_server::serialize::{engine, serialize_plan};
use aries_model::extensions::SavedAssignment;
use aries_planners::solver;
use aries_planners::solver::Metric;
use aries_planning::chronicles::analysis::hierarchical_is_non_recursive;
use aries_planning::chronicles::FiniteProblem;
use async_trait::async_trait;
use clap::Parser;
use futures_util::StreamExt;
use prost::Message;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use unified_planning as up;
use unified_planning::metric::MetricKind;
use unified_planning::unified_planning_server::{UnifiedPlanning, UnifiedPlanningServer};
use unified_planning::{PlanGenerationResult, PlanRequest};
use up::Problem;

/// Server arguments
#[derive(Parser, Default, Debug)]
#[clap(about = "Unified Planning Server")]
struct Args {
    /// Address to listen on
    #[clap(short, long, default_value = "0.0.0.0:2222")]
    address: String,

    #[clap(short, long)]
    /// Encoded UP problem to solve. Optional if a problem is provided in a request.
    file_path: Option<String>,
}

/// Solves the given problem, giving any intermediate solution to the callback.
pub fn solve(problem: &up::Problem, on_new_sol: impl Fn(up::Plan) + Clone) -> Result<up::PlanGenerationResult, Error> {
    let strategies = vec![];
    let htn_mode = problem.hierarchy.is_some();

    ensure!(problem.metrics.len() <= 1, "Unsupported: multiple metrics provided.");
    let metric = if let Some(metric) = problem.metrics.get(0) {
        match up::metric::MetricKind::from_i32(metric.kind) {
            Some(MetricKind::MinimizeActionCosts) => Some(Metric::ActionCosts),
            Some(MetricKind::MinimizeSequentialPlanLength) => Some(Metric::PlanLength),
            Some(MetricKind::MinimizeMakespan) => Some(Metric::Makespan),
            _ => bail!("Unsupported metric kind with ID: {}", metric.kind),
        }
    } else {
        None
    };

    let base_problem = problem_to_chronicles(problem)?;

    let max_depth = u32::MAX;
    let min_depth = if htn_mode && hierarchical_is_non_recursive(&base_problem) {
        max_depth // non recursive htn: bounded size, go directly to max
    } else {
        0
    };

    // callback that will be invoked each time an intermediate solution is found
    let on_new_solution = |pb: &FiniteProblem, ass: Arc<SavedAssignment>| {
        let plan = serialize_plan(problem, pb, &ass);
        match plan {
            Ok(plan) => on_new_sol(plan),
            Err(err) => eprintln!("Error when serializing intermediate plan: {}", err),
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
    )?;
    if let Some((finite_problem, plan)) = result {
        println!(
            "************* PLAN FOUND **************\n\n{}",
            solver::format_plan(&finite_problem, &plan, htn_mode)?
        );
        let status = if metric.is_some() {
            up::plan_generation_result::Status::SolvedOptimally
        } else {
            up::plan_generation_result::Status::SolvedSatisficing
        };
        let plan = serialize_plan(problem, &finite_problem, &plan)?;
        Ok(up::PlanGenerationResult {
            status: status as i32,
            plan: Some(plan),
            metrics: Default::default(),
            log_messages: vec![],
            engine: Some(aries_grpc_server::serialize::engine()),
        })
    } else {
        println!("************* NO PLAN FOUND **************");
        Ok(up::PlanGenerationResult {
            status: up::plan_generation_result::Status::UnsolvableIncompletely as i32,
            plan: None,
            metrics: Default::default(),
            log_messages: vec![],
            engine: Some(engine()),
        })
    }
}
#[derive(Default)]
pub struct UnifiedPlanningService {}

#[async_trait]
impl UnifiedPlanning for UnifiedPlanningService {
    type planOneShotStream = ReceiverStream<Result<PlanGenerationResult, Status>>;

    async fn plan_one_shot(&self, request: Request<PlanRequest>) -> Result<Response<Self::planOneShotStream>, Status> {
        let (tx, rx) = mpsc::channel(32);
        let plan_request = request.into_inner();

        let problem = plan_request
            .problem
            .ok_or_else(|| Status::aborted("The `problem` field is empty"))?;

        let tx2 = tx.clone();
        let on_new_sol = move |plan: up::Plan| {
            let answer = up::PlanGenerationResult {
                status: up::plan_generation_result::Status::Intermediate as i32,
                plan: Some(plan),
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(aries_grpc_server::serialize::engine()),
            };

            // start a new green thread in charge for sending the result
            let tx2 = tx2.clone();
            tokio::spawn(async move {
                if tx2.send(Ok(answer)).await.is_err() {
                    eprintln!("Could not send intermediate solution through the gRPC channel.");
                }
            });
        };

        // run a new green thread in which the solver will run
        tokio::spawn(async move {
            let result = solve(&problem, on_new_sol);
            match result {
                Ok(answer) => {
                    tx.send(Ok(answer)).await.unwrap();
                }
                Err(e) => {
                    tx.send(Err(Status::internal(e.to_string()))).await.unwrap();
                }
            }
        });
        // return the output channel
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn validate_plan(
        &self,
        _request: tonic::Request<up::ValidationRequest>,
    ) -> Result<tonic::Response<up::ValidationResult>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Validation is not supported by the Aries engine.",
        ))
    }

    async fn compile(
        &self,
        _request: tonic::Request<up::Problem>,
    ) -> Result<tonic::Response<up::CompilerResult>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Compilation is not supported by the Aries engine.",
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    // Set address to localhost
    let addr = args.address.as_str().parse()?;
    let upf_service = UnifiedPlanningService::default();

    // If argument is provided, then read the file and send it to the server
    if let Some(file) = args.file_path {
        let problem = std::fs::read(&file)?;
        let problem = Problem::decode(problem.as_slice())?;
        let plan_request = PlanRequest {
            problem: Some(problem),
            ..Default::default()
        };

        let request = tonic::Request::new(plan_request);
        let response = upf_service.plan_one_shot(request).await?;
        let answer = response.into_inner();
        let answer: Vec<_> = answer.collect().await;
        for a in answer {
            println!("{a:?}");
        }
    } else {
        println!("Serving: {}", addr);
        Server::builder()
            .add_service(UnifiedPlanningServer::new(upf_service))
            .serve(addr)
            .await?;
    }

    Ok(())
}
