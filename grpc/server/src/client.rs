use prost::Message;
use upf::upf_client::UpfClient;
use upf::Problem;

pub mod upf {
    pub use aries_grpc_api::*;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = UpfClient::connect("https://127.0.0.1:2222").await?;

    // Get binary data from file as argument
    let buf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| format!("Please provide the problem bin file"));
    let problem = std::fs::read(&buf)?;
    let problem = Problem::decode(problem.as_slice())?;

    let request = tonic::Request::new(problem);

    let response = client.plan(request).await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}
