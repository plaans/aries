use std::fs;

//Build GRPC server and client for UPF planning service
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_file = "src/unified_planning.proto";

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/")
        .compile(&[proto_file], &["src/"])
        .unwrap_or_else(|e| panic!("Failed to compile proto: {}", e));

    fs::rename("src/_.rs", "src/unified_planning.rs")?;

    // Recompile only if the proto file has been modified
    println!("cargo:rerun-if-changed={}", proto_file);

    Ok(())
}
