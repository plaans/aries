# Aries

Aries is a project aimed at exploring constraint-based techniques for automated planning and scheduling.
It is currently under heavy development and generaly should not be considered stable or efficient (even thought many parts are).

## Building

Build relies on `cargo` can be triggered like so:

```
cargo build # debug non-optimized build
cargo build --release ## optimized build
```

This build compile artifacts and place them in the `target/debug` or `target/release` directory.

## Library Modules

This repository contains several crates that provide various functionalities for automated problem solving. In a topological order :

 - `collections`: various collections that are focused on key-value collection where the key can be transformed into an array index.
 - `env_param`: utils to read parameters from environment variables.
 - `backtrack`: Data structures for implementing trails that record event and allow undoing them. This crate provides a trail version meant for internal use in backtrackable structure and one that allows other entities to read the events that were pushed to the queue.
 - `model`: core data struture for representing variables, values, literal and problems.
 - `solver`: Implementation of combinatorial solvers with capabilities from SAT, SMT and CSP solvers. The solver provides an interface that accept additional reasoners.
 - `tnet`: Implementations related to temporal networks.
 - `planning`: A crate that supports manipulating and solving AI PLanning problems.
 - `sat`: A thin wrapper around the `solver` crate that implements a sat executable that solves problems from CNF files.
 - `apps`: A set of binaries that use various capabilities of the other crates.


## Executables

While aries is primarily thought as a library it does come with some programs to allow testing and demonstration. Those include:

- `lcp`: a plan-space planner for PDDL and HDDL, based on a compilation to constraint satisfaction problems..
- `gg`: a state-space planner for PDDL based on heuristic search with the `hadd` heuristic.
- `minisat`: sat solver that mimic the minisat solver
- `jobshop`: lazy-SMT based solver for the jobshop problem 

Source code of these executables can be found in the directory `apps/src/bin`. One can install an executable locally like so (example for `gg`):

```
cargo install --bin gg --path . # should be done in the "apps/" sub-crate
```

## Documentation

An overview of the concepts and algorithms at play in the aries project is provided as an mdbook in the `doc/` folder. To build it, you should first install the [mdBook command line tool](https://rust-lang.github.io/mdBook/index.html).

Then the following command should build the book and open it in a browser. Please refer to the mdbook documentation for on overview of the other features.

```
mdbook build --open doc/
```