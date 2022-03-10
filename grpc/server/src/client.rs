use unified_planning::unified_planning_client::UnifiedPlanningClient;
use unified_planning::PlanRequest;

pub mod unified_planning {
    pub use aries_grpc_api::*;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = UnifiedPlanningClient::connect("https://127.0.0.1:2222").await?;

    let request = tonic::Request::new(PlanRequest::default());

    let response = client.plan_one_shot(request).await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}
