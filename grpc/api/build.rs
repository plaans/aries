//Build GRPC server and client for UPF planning service
#[cfg(feature = "generate_bindings")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    let proto_file = "src/unified_planning.proto";

    let x: [&str; 0] = [];
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/")
        .compile(&[proto_file], &x)
        .unwrap_or_else(|e| panic!("Failed to compile proto: {}", e));

    fs::rename("src/_.rs", "src/unified_planning.rs")?;

    Ok(())
}

#[cfg(not(feature = "generate_bindings"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
