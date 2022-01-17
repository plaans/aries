use async_trait::async_trait;
use tonic::{transport::Server, Request, Response, Status};

mod serialize;
use serialize::*;

use upf::upf_server::{Upf, UpfServer};
use upf::{Answer, Problem};

#[derive(Default)]
pub struct UpfService {}

#[async_trait]
impl Upf for UpfService {
    async fn plan(&self, request: Request<Problem>) -> Result<Response<Answer>, Status> {
        let problem = request.into_inner();

        //Deserialize the problem
        let problem_ = Problem_::deserialize(problem);
        println!("{:?}", problem_);
        let answer = Answer_::serialize(&Answer_::default());
        let response = Response::new(answer);
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set address to localhost
    let addr = "127.0.0.1:2222".parse()?;
    let upf_service = UpfService::default();

    Server::builder()
        .add_service(UpfServer::new(upf_service))
        .serve(addr)
        .await?;

    Ok(())
}
