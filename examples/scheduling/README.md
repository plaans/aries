Very simple solver for disjunctive scheduling problem that exploits the Aries solver.

### Usage

The scheduler is written in Rust so in order to install it you should have a working [rust installation](https://www.rust-lang.org/tools/install).
To compile it you should run:
```shell
cargo build --release --bin scheduler
```
This will produce an executable binary `target/release/scheduler` (target being at the root of this repository).

```shell
./scheduler <problem-kind> <path/to/instance>
```

Common scheduling instances will be found in the `instances/` folder.
```shell
#Solves the first OpenShop instance of Taillard
./target/release/scheduler openshop examples/scheduling/instances/openshop/taillard/tai04_04_01.txt

# Solvers the first JobShop instance of Lawrence
./target/release/scheduler jobshop examples/scheduling/instances/jobshop/la01.txt
```


### Options

```
aries-scheduler 0.1.0

USAGE:
    scheduler [OPTIONS] <kind> <file>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --expected-makespan <expected-makespan>
            When set, the solver will fail if the found solution does not have this makespan

        --lower-bound <lower-bound>                 [default: 0]
    -o, --output <output>                          Output file to write the solution
        --search <search>
            Search strategy to use in {activity, est, parallel} [default: parallel]

        --upper-bound <upper-bound>                 [default: 100000]

ARGS:
    <kind>    Kind of the problem to be solved in {jobshop, openshop}
    <file>    File containing the instance to solve
```

If known beforehand, the makespan of the optimal solution can be specified on the command line (e.g. `--expected-makespan 42`). If the solution found has a different makespan, the solver will exit with error code 1. 