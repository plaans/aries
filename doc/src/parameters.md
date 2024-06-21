# Solver parameters

This document contains a (non-exhaustive) list of the environment variables that can be used to modify some comportment
of the aries solver.

These are read from the environment when first loading the solver and can be use like this:

```sh
ARIES_PRINT_MODEL=true ./cargo run --bin lcp -- problem.pddl
```

| Environment variable           | default | Comment                                                                                                                                                                                                                                                                         |
|--------------------------------|---------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| ARIES_USE_EQ_LOGIC             | false   | Use equality logic theory when interpreting equality over symbolic variables. This is deactivated by default as it may substantially increase the memory consumption of the solver and leading to MEMOUT on problems that are otherwise solved.                                 |
| ARIES_TABLE_STRONG_PROPAGATION | false   | Enables a stronger propagator for table constraints. This is to be used together with equality logic.                                                                                                                                                                           |
| ARIES_LCP_SYMMETRY_BREAKING    | psp     | Which symmetry breaking rule to use by default. This includes `psp` and `simple`. If `psp` is selected but not supported on this problem, it will fall back to `simple`                                                                                                         |
| ARIES_PRINT_MODEL              | false   | If set to true, the chronicle model *after* preprocessing will be printed.                                                                                                                                                                                                      |               
| ARIES_PRINT_RAW_MODEL          | false   | If set to true, the chronicle model *before* preprocessing will be printed.                                                                                                                                                                                                     |               
| ARIES_PRINT_RUNNING_STATS      | false   | Solver would regularly print statistics during solving.                                                                                                                                                                                                                         |
| ARIES_UP_ASSUME_REALS_ARE_INTS | false   | If set to true, the UP backend will interpret any real state variable as an int. It would crash if any non-int value was assigned to it. This is necessary when loading PDDL domains that only allow representing real-valued fluents, even they can only hold integral values. |

Many other variables are available, all starting with `ARIES_` (so you can use grep to find some more).