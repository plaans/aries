A very simple SAT solver implementation based on Aries.

## Usage

The solver takes as input a SAT problem in the CNF format and says whether it is satisfiable or not.

```shell
# Directly run with cargo
cargo run --release --bin aries-sat -- solve <path/to/problem.cnf>

# Build and execute in two steps.
cargo build --release --bin aries-sat
target/release/aries-sat solve <path/to/problem.cnf> # requires previous compilation
```

## Additional options

- the solver provides the subcommands `mus` and `all-mus` to identify which subset(s) of constraints make a problem UNSAT
- You can specify a directory or zip file in which the CNF file will be searched for with `--source <path>` command line option.
- You can specify whether the given problem is SAT (resp. UNSAT) with the command line option `--sat true` (resp. `--sat false`). If the solver find a different answer, it will exit with error code 1.
