//Build GRPC server and client for UPF planning service
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("src/upf.proto")?;
    Ok(())
}
