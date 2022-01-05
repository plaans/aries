Very simple solver for the knapsack problem.

### Usage

```shell
cargo run --release -- instances/01.txt
```

If known beforehand, the value of the optimal solution can be specified on the command line (e.g. `--expected-value 42`). 
If the solution found has a different value, the solver will exit with error code 1. 