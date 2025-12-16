# Automated planning 


This directory contains crates and resources related to the exploitation of the `aries` CP solver in an automated planning context.

## Under development

The planning functionality in Aries is currently being reimplemented in the following three crates. These are still under heavy development and very far from feature complete.

- [`planx`](planx/): A PDDL parser and model for automated planning problems (library, no dependencies on other crates)
- [`timelines`](timelines/): A scheduling-oriented high-level model for planning problems targeting the `aries` CP solver (library)
- [`engine`](engine/): CLI tool for dealing with PDDL planning problems (parsing, solving, ...). At a high level, it parses the problems with `planx` and solves them with `aries-timelines`


## Previous implementation

A previous implementation is provided in the following directories that served as a based for the previous planners (e.g. that participated in the 2023 IPC).
It will be kept here until the implementation reaches feature-parity.

- [`planning`](planning/): Library for handling temporal/numeric/hierarchical planning task based on a representation as chronicles.
- [`planners`](planners/): CLI tool implementing a standalone planning tool.


### Unified-planning planning plugin

This implementation also support the unified-planning integration of Aries:

- the [`unified`](unified/) subdirectory contains the source for the `up-aries` python library that provides support for the aries-based planners in the unified planning library
- the [`grpc`](grpc/) directory contains two crates that unable the rust gRPC server used in the python library.
