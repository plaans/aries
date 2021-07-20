A simple forward search planner for PDDL based on the *hadd* heuristic. The solver is essentially a reimplementation of YAHSP2 for non-temporal PDDL.

## Usage 

You should specify the path of PDDL problem file to the executable. 
`gg` will automatically attempt to find the corresponding domain (but if it fails to do so, you can specify it with `--domain` option).

```shell
cargo run --release -- ../../problems/pddl/gripper/problem.pddl
```