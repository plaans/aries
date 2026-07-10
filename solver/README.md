

# Aries-solver library

This crate provides a library for general purpose constraint programming that is used as backend in several applications, most notably scheduling and automated planning.


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

## Library modules

 - `prelude`: re-exports all common types and traits. In most cases `use aries_solver::prelude::*;` is all you need.
 - `core`: Low-level representation of variables, domains, literals. It also provides the implementation for the state (collection of domains) supporting backtracking and explanations.
 - `model`: Higher-level data structures and API to represent variables, expressions and their combination into constraint satisfaction problems.
 - `solver`: Implementation of combinatorial solvers with capabilities from SAT, SMT and CSP solvers. The solver provides an interface that accepts additional reasoners.
 - `reasoners`: Specialized reasoners that provide inference capabilities to main solver. It currently includes a reasoner for difference logic and one experimental CP-like module.
 - `backtrack`: Data structures for implementing trails that record event and allow undoing them. This crate provides a trail implementation meant for internal use in backtrackable structures and one that allows other entities to read the events that were pushed to the queue.
 - `collections`: Various collections that are focused on key-value collection where the key can be transformed into an array index.


## Example usage

Several examples are available in the `examples/` directory of this crate:
