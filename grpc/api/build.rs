use std::fs;

//Build GRPC server and client for UPF planning service
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = &["src/unified_planning.proto"];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/")
        .compile(proto_files, &["src/"])
        .unwrap_or_else(|e| panic!("Failed to compile proto: {}", e));

    // Recompile only if the proto file has been modified
    for file in proto_files {
        println!("cargo:rerun-if-changed={}", file);
    }

    // Rename the generated files to match the proto file names
    // FIXME: This is a hack, and should be fixed in the future
    for file in fs::read_dir("src/")? {
        let file = file?;
        let path = file.path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        let new_file_name = file_name.replace("_.rs", "unified_planning.rs");
        fs::rename(path.clone(), path.with_file_name(new_file_name))?;
    }
    Ok(())
}
