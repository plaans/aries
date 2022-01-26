use upf::upf_client::UpfClient;
use upf::Problem;

pub mod upf {
    tonic::include_proto!("upf");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = UpfClient::connect("https://127.0.0.1:2222").await?;

    let request = tonic::Request::new(Problem::default());

    let response = client.plan(request).await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}
