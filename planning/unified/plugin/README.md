# Aries integration for unified-planning

[Aries](https://github.com/plaans/aries) is an automated planner targeting hierarchical and temporal problems. 
The objective of Aries is to model and solve hierarchical problems with advanced temporal features and optimization metrics. 
It relies on the proximity of these with scheduling problems to propose a compilation into a constraint satisfaction formalism. 
Solving exploits a custom combinatorial solver that leverages the concept of optional variables in scheduling solvers as well as the clause learning mechanisms of SAT solvers.

This project provides integration of Aries within the [Unified Planning library](https://github.com/aiplan4eu/unified-planning).


## Supported planning approaches 

- *Problem kind*: Hierarchical planning, Temporal planning
- *Operation modes*: Oneshot planning, Plan validation


## Installation

For each aries release, pre-built binaries are available on PyPI and can be installed with: `pip install up-aries`.

[Development builds](https://github.com/plaans/aries/releases/tag/latest) are provided for the HEAD of the master branch and can be installed with: 
```
pip install --force https://github.com/plaans/aries/releases/download/latest/up_aries.tar.gz
```



## Development

A boolean environment variable `UP_ARIES_DEV` allows to automatically recompile Aries from the sources.

