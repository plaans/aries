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

## Library Modules

 - `sat`: reimplementation of the minisat solver, with initial work for lazy SMT usage
 - `stn`: algorithms for Simple Temporal Networks (aka Difference Logic) reasoning.
 - `planning`: module for reasoning on temporal and hierachical planning problems in a chronicle based representation.

## Executables

While aries is primarily thought as a library it does come with some programs to allow testing and demonstration. Those include:

- `gg`: a state-space planner for PDDL based on heuristic search with the `hadd` heuristic.
- `minisat`: sat solver that mimic the minisat solver
- `jobshop`: lazy-SMT based solver for the jobshop problem 
- `pddl2chronicles`: a tool to convert PDDL into a JSON representation of chronicles

Source code of these executables can be found in the directory `src/bin`. One can install an executable locally like so (example for `gg`):

```
cargo install --bin gg --path .
``` 