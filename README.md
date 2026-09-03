# Aries

Aries is a project aimed at exploring constraint-programming techniques for [automated planning and scheduling](https://en.wikipedia.org/wiki/Automated_planning_and_scheduling).
It relies on an original implementation of a constraint solver with optional variables and clause learning to which various scheduling or automated planning problems can be submitted.


## Constraint Programming Solver

**Crate: [`aries-solver`](https://github.com/plaans/aries/tree/master/solver)**

Previous research has shown that while obtaining state-of-the-art performance with constraint-based approaches to planning is possible, it requires pushing the inference capabilities of existing combinatorial solver beyond their current capabilities.
The Aries project thus provides an innovative combinatorial solver that is built from the ground up by (1) mixing several techniques from constraint programming and automated reasoning, and (2) providing original representations and technologies relevant for automated planning:


- **Finite domain CSP**: at the core of a model are discrete variables with a finite domain denoting the set of possible values they can take.
- Various **inference engines** are provided in the solver, conceptually similar to the *theories* of SMT solvers and bringing support for e.g. clauses, simple temporal networks or arbitrary CP propagators.
- **Explanation and Clause learning** are a central component in the solver.
- native support for **optional variables**: some variables can be optional: their presence in the solution will be determined by the value of a literal.
  This allows eager reasoning and constraint propagation by decoupling the presence literal and the domain of the variable.

While the aries solver library is built with automated planning problems in mind, it remains a general purpose solver that can be used for other combinatorial problems.
It was in particular shown to have state of the art performance on several scheduling problems (jobshop, openshop, flexible jobshop and variants).


## Automated Planning with Aries

Aries support solving problems in the PDDL and HDDL formats for specifying problems and provides an integration with the [`unified-planning`](https://github.com/aiplan4eu/unified-planning) library.

A planning problem is translated into an internal representation based on *chronicles*: data structures that specify the requirements and effects of an action.
Chronicles allow a rich temporal representation of an action and is especially useful for representing hierarchical problems where an abstract action can be decomposed into finer-grained ones.
This representation allows for quite natural encoding of the planning problem into a constraint satisfaction problem that can be solved with our internal CP solver.

The planning features are split into families of crates:

- `aries-planners` provides automated planning solvers and was initially developed together with the CP solvers. It has been the bedrock of our planning research until 2025 but is not anymore under active development.
- `aries-planning-engine` is a new, under development, solver for PDDL that is builds on the lessons learned to develop a new, more capable solver with a much more comprehensive support for PDDL. It has not reached feature-parity with `aries-planners` yet but is expected to replace it eventually


## Repository Content

The repository in split into multiple components as crates in cargo workspace:

| Crate | Status | Description |
|:-:|:-:|:-|
| **-- CP --** | | |
|[`aries-solver`](https://github.com/plaans/aries/tree/master/solver) | [![crates.io](https://img.shields.io/crates/v/aries-solver.svg)](https://crates.io/crates/aries-solver) | Constraint Programming library, central to all solvers developed in the project.|
|[`aries-scheduler`](https://github.com/plaans/aries/tree/master/examples/scheduling) | - | CLI solver for scheduling with state-of-the-art performance on jobshop, openshop, flexible jobshop. |
|[`aries-sat`](https://github.com/plaans/aries/tree/master/examples/sat) | example | Minimal SAT solver. |
|[`aries-fzn`](https://github.com/plaans/aries/tree/master/aries_fzn) | experimental | Experimental flatzinc interface for aries. |
| **-- Planning --** | | |
|[`aries-planning-engine`](https://github.com/plaans/aries/tree/master/planning/engine) | experimental | APE: CLI for solving (PDDL) planning problems. Features: parsing, solving, validation. Based on `planx` and `timelines`.  |
|[`aries-planx`](https://github.com/plaans/aries/tree/master/planning/planx) | - | Library providing a planning model and a comprehensive PDDL parser. |
|[`aries-timelines`](https://github.com/plaans/aries/tree/master/planning/timelines) | experimental | Overlay on top of `aries-solver` that exposes planning/scheduling primitives for CP (tasks, conditions, effects, ...) |
|[`aries-planners`](https://github.com/plaans/aries/tree/master/planning/planners) | deprecated | CLI solver for PDDL/HDDL. Fully functional but expected to be superseeded by `aries-planning-engine` in the future. |
|[`up-aries`](https://github.com/plaans/aries/tree/master/planning/unified/plugin) | [![pypi.org](https://img.shields.io/pypi/v/up-aries)](https://pypi.org/project/up-aries/) | Integration of `aries-plan` as a backend solver for the [`unified-planning`](https://github.com/aiplan4eu/unified-planning) python library.|
| **-- Utils --** | | |
|[`aries-env-param`](https://github.com/plaans/aries/tree/master/utils/env_param) | [![crates.io](https://img.shields.io/crates/v/aries-env-param.svg)](https://crates.io/crates/aries-env-param) | Utility to allow overriding a solver's internal parameter with environment variables. |
|[`aries-datalog`](https://github.com/plaans/aries/tree/master/utils/datalog) | - | Minimal datalog engine designed for grounding plannning problems. |
|[`aries-bench`](https://github.com/plaans/aries/tree/master/bench/bench) | - | CLI util for processing benchmark results. |
|[`aries-bench-data`](https://github.com/plaans/aries/tree/master/bench/data) | - | Minimal library allowing solvers to export benchmark results in a standard format. |





## Contributors

- Arthur Bit-Monnot (@arbimo): Main author and maintainer
- Roland Godet (@Shi-Raida): support for numeric state-variables in automated planner, plan validator
- Nika Beriachvili (@nrealus): assumptions and incremental solving API, explanations
- Titouan Seraud (@titorau): minizinc interface (flatzinc solver)

Above is the list of persons with recurring contributions, that have contributed significant parts of the libraries. A comprehensive list of all contributors (often for isolated bugfixes or features) is available in the [contributors section](https://github.com/plaans/aries/graphs/contributors?all=1).




## References

*Aries* is developed in an academic context and the project has been the subject and many aspects of the solver are the subject of academic publications.

 - CP solver core and scheduling solver
   - Arthur Bit-Monnot. *Enhancing Hybrid CP-SAT Search for Disjunctive Scheduling* -- ECAI 2023 [🔗](https://hal.science/hal-04174800)
   - Arthur Bit-Monnot. *Revisiting Optional Variables in Lazy Clause Generation Solvers for Flexible Scheduling* -- CP 2026 [🔗](https://doi.org/10.4230/LIPIcs.CP.2026.7)
 - Papers on automated planning behind Aries:
   - Arthur Bit-Monnot, Roland Godet. *Towards Canonical and Minimal Solutions in a Constraint-based Plan-Space Planner* -- ECAI 2025 [🔗](https://hal.science/hal-05226061)
   - Roland Godet, Arthur Bit-Monnot, Charles Lesire-Cabaniols. *When Quality Matters: Constraint Programming for Automated Temporal and Numeric Planning* -- ICTAI 2025 [🔗](https://hal.science/hal-05376434)
   - Arthur Bit-Monnot. *Experimenting with Lifted Plan-Space Planning as Scheduling: Aries in the 2023 IPC* -- 2023 International Planning Competition [🔗](https://hal.science/hal-04174737)
   -  Nika Beriachvili, Arthur Bit-Monnot. *A Constraint Formulation for Domain Repair with Ground or Lifted Test Plans* -- ICAPS 2026 [🔗](https://hal.science/hal-05581113v2)
   - Roland Godet, Arthur Bit-Monnot. *Chronicles for Representing Hierarchical Planning Problems with Time* -- ICAPS Hierarchical Planning Workshop (HPlan) [🔗](https://hal.science/hal-03690713)

## License

Licensed under either of *Apache License, Version 2.0* or *MIT license* at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this repository by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
