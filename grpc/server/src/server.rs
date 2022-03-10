pub mod unified_planning {
    pub use aries_grpc_api::*;
}

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

use aries_grpc_api::Answer;
use futures_core::Stream;
use unified_planning::unified_planning_server::{UnifiedPlanning, UnifiedPlanningServer};
use unified_planning::PlanRequest;

use async_trait::async_trait;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

// mod serialize;
// mod solver;
// use serialize::*;
// use solver::solve;
// use crate::solver::*;

#[derive(Default)]
pub struct UnifiedPlanningService {
    answers: Arc<Vec<Answer>>,
}

#[async_trait]
impl UnifiedPlanning for UnifiedPlanningService {
    async fn plan_one_shot(&self, request: Request<PlanRequest>) -> Result<Response<Answer>, Status> {
        let answer = self.answers.get(0).unwrap().clone();
        for answer in self.answers.iter() {
            return Ok(Response::new(answer.clone()));
        }
        Ok(Response::new(ReceiverStream::new(Receiver::from(vec![answer]))))
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
