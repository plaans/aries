use aries_grpc_api::{PlanRequest, Problem};
use prost::Message;
use unified_planning::unified_planning_client::UnifiedPlanningClient;

pub mod unified_planning {
    pub use aries_grpc_api::*;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = UnifiedPlanningClient::connect("https://127.0.0.1:2222").await?;

    // Get binary data from file as argument
    let buf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Please provide the problem bin file".to_string());
    let problem = std::fs::read(&buf)?;
    let problem = Problem::decode(problem.as_slice())?;
    let plan_request = PlanRequest {
        problem: Some(problem),
        ..Default::default()
    };

    let request = tonic::Request::new(plan_request);

    let response = client.plan_one_shot(request).await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}
