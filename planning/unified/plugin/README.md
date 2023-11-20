# Aries integration for unified-planning

[Aries](https://github.com/plaans/aries) is an automated planner targeting hierarchical and temporal problems. 
The objective of Aries is to model and solve hierarchical problems with advanced temporal features and optimization metrics. 
It relies on the proximity of these with scheduling problems to propose a compilation into a constraint satisfaction formalism. 
Solving exploits a custom combinatorial solver that leverages the concept of optional variables in scheduling solvers as well as the clause learning mechanisms of SAT solvers.

This project provides integration of Aries within the [Unified Planning library](https://github.com/aiplan4eu/unified-planning).


## Aries solver

The Aries solver supports **action-based planning**, **hierarchical planning** and **scheduling**, with the following features:

 - **classical**: basic action models with symbolic state variables
 - **numeric**: support simple numeric planning over *integer* state variables. 
 - **temporal**: durative actions, intermediate conditions and effects, timed initial literals, timed goals. Support continuous and discrete time semantics. 
 - **optimization**: support for optimizing plans for metrics: `plan-length`, `makespan`, `action-costs` 


Provided engines:

 - **aries**:  
   - **oneshot**: Will return the first plan found, regardless of its quality.
   - **anytime**: Will return a stream of plan of increasing quality until it finds a provably optimal plan (or runs out of time).
 - **aries-opt**:
   - **oneshot**: Will return a provably optimal plan. Note that this may never happen for cases where the plan-space is not bounded (action-based planning and recursive HTN planning).

Note that if you are interested in plan quality, you should consider the `anytime` operation mode of `aries`. This mode will return solutions of increasing quality as they are discovered. On the other end, `aries-opt` will only give you the best solution *once it has shown that no better solutions exist*, which may take a very long additional time (or never happen).


### Limitations

- The focus of Aries is on temporal hierarchical planning and scheduling. While it can in theory solve non-hierarchical (action-based) problems, very little attention as been paid to this setting and it is unlikely to exhibit good performance on non trivial problems.
- Unlike most solvers, Aries searches in plan-space which is not bounded for action-based planning and recursive HTN planning. As a consequence, in these two settings, Aries will not give you an optimality proof or a proof of unsolvability as it will never exhaust its search space.
- Numeric planning in Aries is limited to integers. This is rarely limiting as most fixed precision numerals can be scaled to an integer value. However, note that a problem parsed from PDDL will always be considered to have real-valued state variables (even if only integers are used for it) and thus will be detected as not supported.

## Aries validator

Another engine provided in the `up-aries` package is `aries-val`, a plan validator that covers most features of the UP (far beyond what is support by the aries solver).

Its implementation is mostly independent of the Aries solver.


## Installation

For each release of Aries, pre-built binaries are available on PyPI and can be installed with: 

    pip install up-aries

You can force an upgrade to the latest released version with:

    pip install --upgrade up-aries

[Development builds](https://github.com/plaans/aries/releases/tag/latest) are provided for the HEAD of the master branch and can be installed with: 
```
pip install --force https://github.com/plaans/aries/releases/download/latest/up_aries.tar.gz
```



## Development

The plugin is developed within the main repository of Aries and is meant to co-evolve with it.

If the environment variable `UP_ARIES_DEV` is set to `true`, the plugin will automatically build the `aries` binary from source and use the resulting binary directly. 

The [`dev.env`](../dev.env) file is used to set up a development environment by 1) setting the `UP_ARIES_DEV` environment variable, 2) setting the python path to use the local `up-aries` module and the local [`unified-planning`](../deps/unified-planning) version from the git submodule:

    source dev.env  # assuming bash is used as the shell



## References

- [Arthur Bit-Monnot. *Experimenting with Lifted Plan-Space Planning as Scheduling: Aries in the 2023 IPC*. Proceedings of the 2023 International Planning Competition (IPC), 2023.](https://hal.science/hal-04174737v1/document)
  - High-level overview of the Aries planner, at the time of its participation in the 2023 IPC  
- [Arthur Bit-Monnot. *Enhancing Hybrid CP-SAT Search for Disjunctive Scheduling*,  European Conference on Artificial Intelligence (ECAI), 2023.](https://hal.science/hal-04174800v1/document)
  - Presentation of the combinatorial solver used as a backend
- [Roland Godet, Arthur Bit-Monnot. *Chronicles for Representing Hierarchical Planning Problems with Time*. ICAPS Hierarchical Planning Workshop (HPlan), Jun 2022](https://hal.science/hal-03690713v1/document)
  - Overview of the encoding used to compile planning problems as CSPs. 
