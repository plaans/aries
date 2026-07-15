# Aries

Aries is a project aimed at exploring constraint-based techniques for [automated planning and scheduling](https://en.wikipedia.org/wiki/Automated_planning_and_scheduling).
It relies on an original implementation of constraint solver with optional variables and clause learning to which various automated planning problems can be submitted.


## Constraint Programming Solver

Previous research, especially with the [FAPE](https://github.com/laas/fape) solver, have shown that while obtaining state-of-the-art performance with constraint-based approaches to planning is possible, it requires pushing the inference capabilities of existing combinatorial solver beyond their current capabilities.
The Aries project thus provides an innovative combinatorial solver that is built from the ground up by (1) mixing several techniques from constraint programming and automated reasoning, and (2) providing original representations and technologies relevant for automated planning:


- **Finite domain CSP**: at the core of a model are discrete variables with a finite domain denoting the set of possible values they can take.
- Literals represent expressions on the bounds of variables, for instance `(x <= 11)` or `(y > 10)`.
  This generalizes the literals in SAT solvers to non-boolean variables (whereas SMT or CSP solver typically maintain a correspondence table between such expressions and literals).
- Various **inference engines** are provided in the solver:
  - SAT based engine for disjunctive constraints (*clauses* in which at least one literal must be true), relying on unit clause propagation
  - Difference Logic engine (aka STN), for propagating temporal constraints or general difference constraints between two variables.
    The difference logic engine notably supports forward checking (or theory propagation) and native reasoning on optional variables.
  - General purpose CP engine for adding arbitrary constraints and the associated propagators (linear, max, no-overlap, ...).
- **Explanation and Clause learning** are supported by the various engine.
  When a conflict is detected during search, a new constraint will be inferred that prevents the solver of doing the same mistake.
- **Optional variables**: some variables can be optional: their presence in the solution will be determined by the value of a literal.
  This allows eager reasoning and constraint propagation by decoupling the presence literal and the domain of the variable.

While the aries solver library is built with automated planning problems in mind, it remains a general purpose solver that can be used for other combinatorial problems.
In particular, we provide solvers for [SAT](https://en.wikipedia.org/wiki/Boolean_satisfiability_problem) and [scheduling](https://en.wikipedia.org/wiki/Job-shop_scheduling) problems as a thin wrapper around the library and can be used for testing or demonstration purposes.


## Planning with Aries

Aries support problems in the PDDL and HDDL formats for specifying problems.
A planning problem is then translated into an internal representation based on *chronicles*: data structures that specify the requirements ond effects of an action.
Chronicles allow a rich temporal representation of an action and is especially useful for representing hierarchical problems where an abstract action can be decomposed into finer-grained ones.

This representation allows for quite natural encoding of the planning problem into a constraint satisfaction problem that can be solved with our own combinatorial solver.

The current focus of the solver is on *hierarchical planning* which is especially well suited to represent various robotic and scheduling problems. Non-hierarchical problems are supported but do require more work to reach a state-of-the-art performance (areas of improvement notably include better symmetry breaking constraints and search heuristics).


# Usage

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

 - `solver`: contains the `aries_solver` crate with the core library for the CP solver (CSP model, propagators, search, ...)
 - `env_param`: Utils to read parameters from environment variables, used in various places to allow changing the default parameters of the solver
 - `planning`: Several crates related to automated planning. In particular:
   - `planning/planners`: provides a complete PDDL and HDDL planner, with support for temporal and numeric models
   - `planning/unified/plugin`: provides the `up-aries` python module (published on PyPI) that allows using aries as a backend for the [unified-planning](https://github.com/aiplan4eu/unified-planning) library
   - `validator`: a plan validator for the unified-planning library
 - `aries_fzn`: implements a solver for the `flatzinc` constraint modelling language, supporting the integration of Aries as a backend for [`minizinc`](https://www.minizinc.org/)
 - `examples`: Several thin wrappers around the library to demonstrate and test its capabilities. Notably:
   - `sat`: A thin wrapper around the `solver` crate that implements a sat executable that solves problems from CNF files.
   - `scheduler`: A solver for several disjunctive scheduling problems
   - `knapsack`: an example solver for knapsack problems
 - `utils`: several utility crates providing target-scope features used in other crates



## Executables

While aries is primarily thought as a library it does come with some programs to allow testing and demonstration. Those include:

- `aries-plan`: a plan-space planner for PDDL and HDDL, based on a compilation to constraint satisfaction problems.
- `scheduler`: solver for the jobshop and openshop problems
- `aries-sat`: SAT solver that mimic the minisat solver behavior

Source code of these executables can be found in the directory `apps/src/bin`. One can install an executable locally like so (example for `gg`):

```shell
cargo install --bin scheduler --path . # should be done in the "apps/" sub-crate
```

These can also be run directly with `cargo`. For instance for `gg`:

```shell
cargo run --release --bin scheduler -- <arguments>
```



## Contributors

- Arthur Bit-Monnot (@arbimo): Main author, maintainer
- Roland Godet (@Shi-Raida): support for numeric state-variables in automated planner, plan validator
- Nika Beriachvili (@nrealus): assumptions and incremental solving API, explanations
- Titouan Seraud (@titorau): minizinc interface (flatzinc solver)

Above is the list of persons with recurring contributions, that have contibuted significant parts of the libraries. A comprehensive list of all contributors (often for isolated bugfixes or features) is available in the [contributors section](https://github.com/plaans/aries/graphs/contributors).



## References

 - CP solver core and scheduling solver
   - Arthur Bit-Monnot. *Enhancing Hybrid CP-SAT Search for Disjunctive Scheduling* -- ECAI 2023 [(link)](https://hal.science/hal-04174800)
   - Arthur Bit-Monnot. *Revisiting Optional Variables in Lazy Clause Generation Solvers for Flexible Scheduling* -- CP 2026 [(link)](https://doi.org/10.4230/LIPIcs.CP.2026.48)


## License

Licensed under either of *Apache License, Version 2.0* or *MIT license* at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this repository by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
