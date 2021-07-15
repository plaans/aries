# Aries

Aries is a project aimed at exploring constraint-based techniques for automated planning and scheduling. 
It relies on an original implementation of constraint solver with optional variables and clause learning to which various automated planning problems can be submitted.

## Building

First make sure you have [rust installed on your machine](https://www.rust-lang.org/tools/install).
Build relies on `cargo` can be triggered like so:

```shell
cargo build            # debug non-optimized build
cargo build --release  # optimized build
```

This compiles artifacts and place them in the `target/debug` or `target/release` directory.

## Library Modules

This repository contains several crates that provide various functionalities for automated problem solving. In a topological order :

 - `collections`: Various collections that are focused on key-value collection where the key can be transformed into an array index.
 - `env_param`: Utils to read parameters from environment variables.
 - `backtrack`: Data structures for implementing trails that record event and allow undoing them. This crate provides a trail implementation meant for internal use in backtrackable structures and one that allows other entities to read the events that were pushed to the queue.
 - `model`: Core data structures for representing variables, values, literal and problems.
 - `solver`: Implementation of combinatorial solvers with capabilities from SAT, SMT and CSP solvers. The solver provides an interface that accepts additional reasoners.
 - `tnet`: Implementations related to temporal networks.
 - `planning`: A crate that supports manipulating and solving AI PLanning problems.
 - `sat`: A thin wrapper around the `solver` crate that implements a sat executable that solves problems from CNF files.
 - `apps`: A set of binaries that exploit various capabilities of the other crates.


## Executables

While aries is primarily thought as a library it does come with some programs to allow testing and demonstration. Those include:

- `lcp`: a plan-space planner for PDDL and HDDL, based on a compilation to constraint satisfaction problems..
- `gg`: a state-space planner for PDDL based on heuristic search with the `hadd` heuristic.
- `minisat`: sat solver that mimic the minisat solver behavior
- `jobshop`: lazy-SMT based solver for the jobshop problem 

Source code of these executables can be found in the directory `apps/src/bin`. One can install an executable locally like so (example for `gg`):

```shell
cargo install --bin gg --path . # should be done in the "apps/" sub-crate
```

These can also be run directly with `cargo`. For instance for `gg`:

```shell
cargo run --release --bin gg -- <arguments>
```

## Documentation

An overview of the concepts and algorithms at play in the aries project is provided as an mdbook in the `doc/` folder. To build it, you should first install the [mdBook command line tool](https://rust-lang.github.io/mdBook/index.html).

Then the following command should build the book and open it in a browser. Please refer to the mdbook documentation for on overview of the other features.

```
mdbook build --open doc/
```