use prost::Message;
use unified_planning::unified_planning_client::UnifiedPlanningClient;
use unified_planning::PlanRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = UnifiedPlanningClient::connect("https://127.0.0.1:2222").await?;

    // Get binary data from file as argument
    let buf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Please provide the problem bin file".to_string());
    let problem = std::fs::read(&buf)?;
    let problem = unified_planning::Problem::decode(problem.as_slice())?;
    let plan_request = PlanRequest {
        problem: Some(problem),
        ..Default::default()
    };

    let request = tonic::IntoRequest::into_request(plan_request);

    let response = client.plan_one_shot(request).await?;

    let mut response = response.into_inner();
    while let Some(msg) = response.message().await? {
        println!("GOT: {:?}", &msg);
        for log in msg.log_messages {
            println!("  [{}] {}", log.level, log.message);
        }
    }

    Ok(())
}
