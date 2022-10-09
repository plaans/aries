Very simple solver for the jobshop scheduling problem.

### Usage

```shell
cargo run --release -- instances/la01.txt
```

If known beforehand, the makespan of the optimal solution can be specified on the command line (e.g. `--expected-makespan 42`). If the solution found has a different makespan, the solver will exit with error code 1. 