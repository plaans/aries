# In-process Aries engines for unified-planning

This package provides [unified-planning](https://github.com/aiplan4eu/unified-planning) engines for the Aries planner.
The planner runs in the Python process: a native module built with [PyO3](https://pyo3.rs) reads the unified-planning problem objects directly.
Unlike the `up_aries` integration (`planning/unified/plugin`), it starts no gRPC server or subprocess.

| Engine           | Class          | Operation modes  | Guarantees                       |
|------------------|----------------|------------------|----------------------------------|
| `aries-pyo3`     | `AriesPyO3`    | oneshot, anytime | anytime: increasing quality      |
| `aries-pyo3-opt` | `AriesPyO3Opt` | oneshot          | optimal plans (not for action-based problems) |

The engines support the same problem kinds as `aries` and `aries-opt`: action-based, hierarchical and scheduling problems.
They also accept a warm-start plan (`warm_start_plan`).

## Installation

Building the package requires a Rust toolchain.

```shell
pip install ./planning/python
```

The package requires a development release of unified-planning. If pip does not select one, install unified-planning first, either from the submodule (`pip install ./planning/unified/deps/unified-planning`) or with `pip install --pre unified-planning`.

For development, `maturin develop --profile python` (run from `planning/python`) builds the module and installs the package in editable mode in the current virtual environment.

## Usage

The engines must be registered in the unified-planning factory:

```python
from unified_planning.environment import get_environment
from unified_planning.shortcuts import OneshotPlanner

env = get_environment()
env.factory.add_engine("aries-pyo3", "up_aries_pyo3", "AriesPyO3")
env.factory.add_engine("aries-pyo3-opt", "up_aries_pyo3", "AriesPyO3Opt")

with OneshotPlanner(name="aries-pyo3", params={"max_depth": 10}) as planner:
    result = planner.solve(problem, timeout=10)
```

Alternatively, add the engines to an `up.ini` file (see `Factory.configure_from_file` in unified-planning):

```ini
[engine aries-pyo3]
module_name: up_aries_pyo3
class_name: AriesPyO3

[engine aries-pyo3-opt]
module_name: up_aries_pyo3
class_name: AriesPyO3Opt
```

Parameters:

- `min_depth` (default `0`) and `max_depth` (default unbounded): bounds on the depth of the search.
- `strategies`: list (or comma-separated string) of search strategies among `forward`, `activity`, `activity-bool`, `activity-bool-light` and `causal`. By default, a portfolio of strategies runs in parallel.

The environment variables of Aries (e.g. `ARIES_WARM_UP` or `ARIES_LCP_SYMMETRY_BREAKING`) are read once, on first use. They must be set before the first resolution in the process.

## Limitations

- The solver cannot be interrupted: a resolution stops on a definitive answer or at the timeout. `Ctrl-C` only takes effect once the solver returns.
- The output of the solver (on file descriptors 1 and 2) is redirected to `output_stream` at the end of the resolution, or discarded if `output_stream` is `None`. The redirection is process-wide, so the engines should not be used concurrently in the same process.
- When the consumer of an anytime resolution stops iterating early, the solver keeps running in a background thread until its timeout or a definitive answer.
- The plan validator (`aries-val`) is not provided yet. Use the `up_aries` integration for validation.

## Implementation

- `src/convert.rs`: conversion of the problem into chronicles, a port of `planning/grpc/server/src/chronicles.rs` with the same functions in the same order. Expressions are first converted (`src/up.rs`) to the shape of their protobuf counterpart so that their handling is unchanged.
- `src/serialize.rs` and `src/warm_up.rs`: ports of the corresponding files of the gRPC server.
- `src/planner.rs`: resolution, a port of `solve_blocking` of the gRPC server.
- `python/up_aries_pyo3/engine.py`: the unified-planning engines, which build the plans from the plain data returned by the native module.
