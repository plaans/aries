

# Aries-solver library

This crate provides a library for general purpose constraint programming that is used as backend in several applications, most notably scheduling and automated planning.


## Library modules

 - `core`: Low-level representation of variables, domains, literals. It also provides the implementation for the state (collection of domains) supporting backtracking and explanations.
 - `model`: Higher-level data structures and API to represent (typed) variables, expressions and their combination into constraint satisfaction problems.
 - `collections`: Various collections that are focused on key-value collection where the key can be transformed into an array index.
 - `env_param`: Utils to read parameters from environment variables.
 - `backtrack`: Data structures for implementing trails that record event and allow undoing them. This crate provides a trail implementation meant for internal use in backtrackable structures and one that allows other entities to read the events that were pushed to the queue.
 - `solver`: Implementation of combinatorial solvers with capabilities from SAT, SMT and CSP solvers. The solver provides an interface that accepts additional reasoners.
 - `reasoners`: Specialized reasoners that provide inference capabilities to main solver. It currently includes a reasoner for difference logic and one experimental CP-like module.







## Documentation

Documentation can be built with cargo doc:

```
cargo doc [--document-private-items]
```
