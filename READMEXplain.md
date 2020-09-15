# Aries

Aries is a project aimed at exploring constraint-based techniques for automated planning and scheduling.
It is currently under heavy development and generaly should not be considered stable or efficient (even thought some part are).

## Building

Build relies on `cargo` can be triggered like so:

```
cargo build # debug non-optimized build
cargo build --release ## optimized build
```

This build compile artifacts and place them in the `target/debug` or `target/release` directory.

## Executables

While aries is primarily thought as a library it does come with some programs to allow testing and demonstration. Those include:

- `gg`: a state-space planner for PDDL based on heuristic search with the `hadd` heuristic. You can create plan with the one in this project.
- `minisat`: sat solver that mimic the minisat solver
- `jobshop`: lazy-SMT based solver for the jobshop problem 
- `pddl2chronicles`: a tool to convert PDDL into a JSON representation of chronicles
- `explainable`: a tool to create explaination for a plan from a PDDL problem.

Source code of these executables can be found in the directory `apps/src/bin`. One can install an executable locally like so (example for `gg`):

```
cargo install --bin gg --path . # should be done in the "apps/" sub-crate
``` 

To use explainable you can do 
./target/release/explainable <way/to/problem.pddl> <way/to/plan> <Options>

Options available for explainable
-d domain           to give manually the domain of the problem
-s                  to create dot file for support
-m                  to create dot file for menace
-t                  to create dot file for temporal representation
-q question         Ask question
Form of the question "question parameters"
Questions available:
    -support (1 parameter)
    -supported (1 parameter)
    -goal (1 parameter)
    -necessary (1 parameter)
    -waybetween (2 parameter)
    -menace (2 parameter)
    -betweeness (1 parameter)
    -synchro (as you want parameters)
    -parallelizable (2 parameters)
Example of question : "support 4"

-i                  interactive mode
    s   dot support + affichage matrice support
    m   dot menace + affichage matrice menace
    q   question same format as option without "" 
    gg  refaire plan
    p   affichage plan
    e   exit

if you want to use your dot file to make a graph:
dot -Tpng graphique.dot -o graph.png