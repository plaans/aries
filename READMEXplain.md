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
-d domain           # To give manually the domain of the problem
-s                  # To create dot file for support
-m                  # To create dot file for threat
-t                  # To create dot file for temporal representation
-q question         # Ask question

Form of the question "question parameters"
Questions available:
    -support <step>                             #Display others steps support by step 
    -supported <step>                           #Display others steps support of step
    -goal <step>                                #Display true if step accomplish a goal
    -necessary <step>                           #Display if step participates to the accomplishment of a goal, necessary-d to have the shortest path
    -path <source-step> <target-step>           #Display path between two steps, path-d to have the path.
    -threat <source-step> <target-step>         #Display if source step threat target-step if it put right before.
    -betweeness <n-score>                       #Display all step with a betweeness upper than the n-th score.
    -synchro <parameters>                       #Display step that make link between group based on parameters
    -parallelizable <step> <step>               #Display a boolean to know if the two steps are parallelizable, parallelizable-d to have more detail
Example of question : "support 4"

-i                  # interactive mode
    s   Generate dot support and display matrix support
    m   Generate dot threat and display matrix threat
    q   Question same format as option without "" 
    gg  Make plan with aries planificator if you have suspicion about your plan
    p   Display plan
    h   Help
    e   exit

if you want to use your dot file to make a graph:
dot -Tpng graphique.dot -o graph.png
Or
xdot