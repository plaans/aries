# GRPC

This module holds the functionality for the gRPC library integrated into Aries. The implementation is currently focused towards [**unified_planning**](https://github.com/aiplan4eu/unified-planning).

## GRPC API

To generate rust bindings for the unified planning [protobuf definition](./api/src/unified_planning.proto), run the following command:

```bash
cargo build --features=generate_bindings
```

The generated code will be located in [`api/src/unified_planning.rs`](./api/src/unified_planning.rs).

If you are experiencing any issues with building/updating the rust definitions, perform `cargo clean` and try again.

## UP Server

The UP server is a gRPC server that can be used with unified planning.

Currently the server can be run in two modes:

- The server can take in a single encoded file and process it. Once the problem is solved, send the plan back to **unified_planning**. This is useful for testing.
- The server can wait for a problem description request, process it, solve it and send back the solution respecting the protobuf definition.

### Usage

To build the server, run the following command:

```bash
cargo build --features=generate_bindings --bin up-server
```

To run the gRPC server, use the following command:

```bash
# To launch the server directly at 0.0.0.0:2222
cargo run --bin up-server

# To launch the server with a custom port
cargo run --bin up-server -- --address 0.0.0.0:50051

# To launch the server with a problem file
cargo run --release --bin up-server -- --file-path <root-of-repo>/ext/up/bins/problems/matchcellar.bin

# To launch the server with a problem file and a custom port
cargo run --release --bin up-server -- --address 0.0.0.0:50051 --file-path <root-of-repo>/ext/up/bins/problems/matchcellar.bin
```

More example problems are available in [this directory](../ext/up/bins/).
