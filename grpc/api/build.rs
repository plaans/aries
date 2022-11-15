use std::fs;

//Build GRPC server and client for UPF planning service
fn build_definitions() -> Result<(), Box<dyn std::error::Error>> {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(feature = "unified_planning") {
        build_definitions()
    } else {
        println!("To compile the unified planning definitions, run cargo build --features=unified_planning");
        Ok(())
    }
}
