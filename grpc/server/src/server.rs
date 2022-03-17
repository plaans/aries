mod chronicles;
use anyhow::Error;
use chronicles::problem_to_chronicles;
pub mod unified_planning {
    pub use aries_grpc_api::*;
}

use aries_grpc_api::Problem;
// use aries_planners::{Option, Planner};

use unified_planning::unified_planning_server::{UnifiedPlanning, UnifiedPlanningServer};
use unified_planning::{Answer, PlanRequest};

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

pub fn solve(problem: &Option<Problem>) -> Result<Vec<Answer>, Error> {
    //TODO: Get the options from the problem
    // let opt = Option::default();
    //TODO: Check if the options are valid for the planner
    // let mut planner = Planner::new(opt.clone());

    // println!("{:?}", problem);
    let problem = problem.clone().unwrap();
    let _spec = problem_to_chronicles(&problem)?;
    // planner.solve(_spec, &opt)?;
    // let answer = planner.get_answer();
    // planner.format_plan(&answer)?;

    let answer = Answer::default();

    Ok(vec![answer])
}
#[derive(Default)]
pub struct UnifiedPlanningService {}

#[async_trait]
impl UnifiedPlanning for UnifiedPlanningService {
    async fn plan_one_shot(&self, request: Request<PlanRequest>) -> Result<Response<Self::planOneShotStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        let plan_request = request.into_inner();

        tokio::spawn(async move {
            if let Ok(answers) = solve(&plan_request.problem) {
                for answer in answers {
                    tx.send(Ok(answer.clone())).await.unwrap();
                }
            } else {
                tx.send(Err(Status::new(tonic::Code::Internal, "solver failed")))
                    .await
                    .unwrap();
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

    Server::builder()
        .add_service(UnifiedPlanningServer::new(upf_service))
        .serve(addr)
        .await?;
    Ok(())
}
