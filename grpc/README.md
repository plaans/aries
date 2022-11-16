# GRPC

This module holds the functionality for the gRPC library integrated into Aries. The implementation is currently focused towards [**unified_planning**](https://github.com/aiplan4eu/unified-planning).

<!-- TODO: Update README for UP Server and Usage Information -->

## GRPC API

To build the grpc API for the `up-server`, run the following command:

```bash
cargo build --features=unified_planning
```

If you are experiencing any issues with building/updating the rust definitions, perform `cargo clean` and try again.

## Information

Currently the server can be run in two modes:

- The server can take in a single file and process it. Once the problem is solved, send the plan back to **unified_planning**. This is useful for testing.
- The server can wait for up-server to send a problem description, process it, solve it and send back the solution.

## Setup

To setup the server for testing, you will use the following:

- The problem description file in `.bin` format. Some problems are available in [this location](../ext/up/bins/). The information related to generating all problems is available at this [location](../ext/up/README.md).

## Usage

To run the gRPC server, use the following command:

```bash
cargo run --release --bin up-server <path-to-binary>
```

or simply,

```bash
cargo run --bin up-server
```

For example:

```bash
cargo run --release --bin up-server ../ext/up/bins/problems/matchcellar.bin
```

## Todo

- [ ] Add support for temporal constraints
- [ ] Validate the server for plans / Unsupported problems
- [ ]Export the server to different architectures
