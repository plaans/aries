use anyhow::Error;
use aries_grpc_server::chronicles::problem_to_chronicles;
pub mod unified_planning {
    pub use aries_grpc_api::*;
}

use aries_grpc_api::Problem;
use aries_grpc_server::serialize::serialize_answer;
use aries_planners::solver;

use prost::Message;
use unified_planning::unified_planning_server::{UnifiedPlanning, UnifiedPlanningServer};
use unified_planning::{Answer, PlanRequest};

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

pub fn solve(problem: &Option<Problem>) -> Result<Vec<Answer>, Error> {
    let mut answers = Vec::new();
    //TODO: Get the options from the problem

    let min_depth = 0;
    let max_depth = 10;
    let strategies = vec![];
    let optimize_makespan = true;
    let htn_mode = false;

    // println!("{:?}", problem);
    let problem = problem.clone().unwrap();
    let base_problem = problem_to_chronicles(&problem)?;
    let result = solver::solve(
        base_problem,
        min_depth,
        max_depth,
        &strategies,
        optimize_makespan,
        htn_mode,
    )?;
    if let Some((finite_problem, plan)) = result {
        println!(
            "************* PLAN FOUND **************\n\n{}",
            solver::format_plan(&finite_problem, &plan, htn_mode)?
        );
        let answer = serialize_answer(&problem, &finite_problem, &Some(plan))?;
        answers.push(answer);
    }
    Ok(answers)
}
#[derive(Default)]
pub struct UnifiedPlanningService {}

#[async_trait]
impl UnifiedPlanning for UnifiedPlanningService {
    async fn plan_one_shot(&self, request: Request<PlanRequest>) -> Result<Response<Self::planOneShotStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        let plan_request = request.into_inner();

        tokio::spawn(async move {
            let result = solve(&plan_request.problem);
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

    type planOneShotStream = ReceiverStream<Result<Answer, Status>>;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        let _answer = response.into_inner();
        // println!("RESPONSE={:?}", answer);
    } else {
        Server::builder()
            .add_service(UnifiedPlanningServer::new(upf_service))
            .serve(addr)
            .await?;
    }

    Ok(())
}
