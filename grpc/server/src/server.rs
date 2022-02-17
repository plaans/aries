use anyhow::{Error, Result};
use async_trait::async_trait;
use prost::Message;
use tonic::{transport::Server, Request, Response, Status};

mod chronicles;
use chronicles::problem_to_chronicles;

use aries_grpc_api::upf_server::{Upf, UpfServer};
use aries_grpc_api::{Answer, Problem};
use aries_planners::{Option, Planner};

pub fn solve(problem: aries_grpc_api::Problem) -> Result<aries_grpc_api::Answer, Error> {
    //TODO: Get the options from the problem
    let opt = Option::default();
    //TODO: Check if the options are valid for the planner
    let mut planner = Planner::new(opt.clone());

    println!("{:?}", problem);
    let _spec = problem_to_chronicles(problem)?;
    planner.solve(_spec, &opt)?;
    let answer = planner.get_answer();
    planner.format_plan(&answer)?;

    let answer = Answer::default();

    Ok(answer)
}

#[derive(Default)]
pub struct UpfService {}

#[async_trait]
impl Upf for UpfService {
    async fn plan(&self, request: Request<Problem>) -> Result<Response<Answer>, Status> {
        let problem = request.into_inner();

        let _answer = solve(problem);
        if let Ok(answer) = _answer {
            let response = Response::new(answer);
            Ok(response)
        } else {
            Err(Status::internal(_answer.unwrap_err().to_string()))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set address to localhost
    let addr = "127.0.0.1:2222".parse()?;
    let upf_service = UpfService::default();

    // Check if any argument is provided
    let buf = std::env::args().nth(1);

    // If argument is provided, then read the file and send it to the server
    if let Some(buf) = buf {
        let problem = std::fs::read(&buf)?;
        let problem = Problem::decode(problem.as_slice())?;
        let request = tonic::Request::new(problem);
        let response = upf_service.plan(request).await?;
        let answer = response.into_inner();
        println!("RESPONSE={:?}", answer);
        if answer.plan == None {
            panic!("Error: Unable to solve the problem");
        }
    } else {
        Server::builder()
            .add_service(UpfServer::new(upf_service))
            .serve(addr)
            .await?;
    }

    Ok(())
}
