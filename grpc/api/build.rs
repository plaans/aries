//Build GRPC server and client for UPF planning service
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/")
        .compile(&["src/unified_planning.proto"], &["src/"])?;
    Ok(())
}
