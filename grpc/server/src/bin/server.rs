use anyhow::{bail, ensure, Error};
use aries_grpc_server::chronicles::problem_to_chronicles;

use aries_grpc_server::serialize::serialize_answer;
use aries_planners::solver;
use unified_planning as up;
use up::Problem;

use unified_planning::unified_planning_server::{UnifiedPlanning, UnifiedPlanningServer};
use unified_planning::{PlanGenerationResult, PlanRequest};

use aries_planners::solver::Metric;
use aries_planning::chronicles::analysis::hierarchical_is_non_recursive;
use async_trait::async_trait;
use futures_util::StreamExt;
use prost::Message;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use unified_planning::metric::MetricKind;

pub fn solve(problem: &up::Problem) -> Result<Vec<up::PlanGenerationResult>, Error> {
    let mut answers = Vec::new();
    //TODO: Get the options from the problem

    let strategies = vec![];
    let htn_mode = problem.hierarchy.is_some();

    ensure!(problem.metrics.len() <= 1, "Unsupported: more than on metric provided.");
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

    let result = solver::solve(base_problem, min_depth, max_depth, &strategies, metric, htn_mode)?;
    if let Some((finite_problem, plan)) = result {
        println!(
            "************* PLAN FOUND **************\n\n{}",
            solver::format_plan(&finite_problem, &plan, htn_mode)?
        );
        let answer = serialize_answer(problem, &finite_problem, &Some(plan))?;
        answers.push(answer);
    } else {
        println!("************* NO PLAN FOUND **************");
    }
    // TODO: allow sending a stream of answers rather that sending the vector
    Ok(answers)
}
#[derive(Default)]
pub struct UnifiedPlanningService {}

#[async_trait]
impl UnifiedPlanning for UnifiedPlanningService {
    type planOneShotStream = ReceiverStream<Result<PlanGenerationResult, Status>>;

    async fn plan_one_shot(&self, request: Request<PlanRequest>) -> Result<Response<Self::planOneShotStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        let plan_request = request.into_inner();

        let problem = plan_request
            .problem
            .ok_or_else(|| Status::aborted("The `problem` field is empty"))?;

        tokio::spawn(async move {
            let result = solve(&problem);
            match result {
                Ok(answers) => {
                    for answer in answers {
                        tx.send(Ok(answer.clone())).await.unwrap();
                    }
                }
                Err(e) => {
                    tx.send(Err(Status::new(tonic::Code::Internal, e.to_string())))
                        .await
                        .unwrap();
                }
            }
        });
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
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    // Set address to localhost
    let addr = "127.0.0.1:2222".parse()?;
    let upf_service = UnifiedPlanningService::default();

    // Check if any argument is provided
    let buf = std::env::args().nth(1);

    // If argument is provided, then read the file and send it to the server
    if let Some(buf) = buf {
        let problem = std::fs::read(&buf)?;
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
