This directory contains some benchmarking utility crates.

- the `data/` directory contains a very minimal crate `aries-bench-data` that defines a data structure for representing a benchmark result (result of a single solver and a single instance) and exporting to a file in stardard format.
- the `bench/` directory contains a much larger crate `aries-bench` from processing results in the standard format, displaying results in the terminal, plots or latex tables.

A solver implementation is expected to depend on the `aries-bench-data` crate and provide cli arguments for exporting its results.


The result format consists of a single `.json` file for each benchmark result. The results of a single solver being grouped in a directory whose name identifies the solver.
For instance, the following command would run two configurations of the scheduling solver and save their results in two distinct directories (set by the `--report` CLI parameter).
```sh
cargo run --release --bin scheduler -- fjs examples/scheduling/instances/flexible/bc/mt*.fjs -t 60 --report /tmp/aries-full
cargo run --release --bin scheduler -- fjs examples/scheduling/instances/flexible/bc/mt*.fjs -t 60 --report /tmp/aries-none --no-overlap none
```

The CLI tool can then be used to process the results:
```sh
cargo run --bin aries-bench -- /tmp/aries-full /tmp/aries-none
# or equivalently, the base directory for the solvers can be specified separately
cargo run --bin aries-bench  -- --base-dir /tmp/ aries-full aries-none 
```
