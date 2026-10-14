# aries-lp

A fast, pure-Rust linear programming solver library with support for dynamic bound modifications, incremental variable/constraint additions, and feasibility checking.

This crate is a fork and enhancement of [`minilp`](https://crates.io/crates/minilp) originally written by [ztlpn](https://github.com/ztlpn/minilp).

[Linear programming](https://en.wikipedia.org/wiki/Linear_programming) is a technique for finding the minimum (or maximum) of a linear function of a set of continuous variables subject to linear equality and inequality constraints.

---

## What's New in this Fork?

Compared to standard `minilp`, this crate introduces:

* **A unified `Lp` type**: the single entry point that owns the linear program and keeps an internal simplex solver in sync.
  Variables, constraints and bound updates applied to an `Lp` are immediately available for the next solve, without rebuilding the problem from scratch.
* **Incremental solving**: tighten or relax variable bounds ([`Lp::set_bound`]) and add new variables or constraints on the fly; subsequent solves start from the previous simplex state.
* **Feasibility checking**: [`Lp::check_feasibility`] quickly verifies that the LP admits at least one solution, ignoring the objective, which is useful for interactive algorithms such as branch-and-bound or SAT/SMT-style search.
* **Infeasibility certificates**: when the constraints cannot be satisfied, the solver can produce a Farkas certificate of infeasibility ([`results::InfeasibilityReason::Certified`]) that can be independently verified with [`Lp::is_certificate_valid`].

---

## Features

* **Pure Rust** implementation.
* Able to solve problems with hundreds of thousands of variables and constraints.
* **Incremental**: add variables/constraints or modify bounds on an existing LP without solving from scratch.
* **Feasibility engine**: dedicated checking mechanism for dynamic constraint satisfaction problem solving.

> **Warning**: Like `minilp`, this library uses floating-point simplex pivoting operations. Harder problems may cycle, suffer numerical instability, or fail with a [`results::SolverFailure`]. Please report bugs and contribute code! Certificates are intended to help you detect false unsatisfiability; however, we do not provide certificates for satisfiability.

---

## Examples

### 1. Basic Problem Solving

```rust
use aries_lp::prelude::*;

// Maximize x + 2 * y subject to x >= 0, 0 <= y <= 3
let mut lp = Lp::new(OptimizationDirection::Maximize);
let x = lp.add_var(1.0, (0.0, f64::INFINITY));
let y = lp.add_var(2.0, (0.0, 3.0));

// Constraints: x + y <= 4 and 2 * x + y >= 2
lp.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Le, 4.0);
lp.add_constraint([(x, 2.0), (y, 1.0)], ComparisonOp::Ge, 2.0);

// Optimal value is 7, achieved at x = 1 and y = 3
match lp.solve().unwrap() {
    SolveStatus::Optimal(solution) => {
        assert_eq!(solution.objective(), 7.0);
        assert_eq!(solution[x], 1.0);
        assert_eq!(solution[y], 3.0);
    }
    _ => unreachable!("this problem has a unique optimal solution"),
}
```

---

### 2. Dynamic Bound Modifications & Incremental Solving

Bounds can be updated between two solves, without re-building the LP from scratch:

```rust
use aries_lp::prelude::*;

let mut lp = Lp::new(OptimizationDirection::Maximize);
let x = lp.add_var(1.0, (0.0, 3.0));
let y = lp.add_var(2.0, (0.0, 3.0));
lp.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Le, 4.0);

// Optimal value is 7, achieved at x = 1 and y = 3
let solution = lp.solve().unwrap().solution().unwrap();
assert_eq!(solution.objective(), 7.0);

// Restrict the upper bound of x to 0.5 and re-solve
lp.set_bound(x, Bound::Upper, 0.5).unwrap();
let solution = lp.solve().unwrap().solution().unwrap();
assert_eq!(solution[x], 0.5);
assert_eq!(solution[y], 3.0);
assert_eq!(solution.objective(), 6.5);
```

---

### 3. Feasibility Checking

`Lp::check_feasibility` is designed for interactive algorithms (e.g. branch-and-bound or SAT/SMT-like engines) that need to push bounds and add variables dynamically while maintaining feasibility.
It ignores the objective function and only answers the question: does this LP admit at least one solution?

```rust
use aries_lp::prelude::*;

let mut lp = Lp::new(OptimizationDirection::Minimize);
let x = lp.add_var(0.0, (0.0, f64::INFINITY));
let y = lp.add_var(0.0, (0.0, 10.0));
lp.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Ge, 5.0);

// Feasible: x = 5, y = 0 satisfies the constraint
assert!(lp.check_feasibility().unwrap().feasible());

// Tightening the upper bound of y keeps the problem feasible
lp.set_bound(y, Bound::Upper, 2.0).unwrap();
assert!(lp.check_feasibility().unwrap().feasible());

// Bounds that are trivially incompatible are rejected
lp.set_bound(x, Bound::Upper, 2.0).unwrap();
assert!(lp.set_bound(x, Bound::Lower, 3.0).is_err());
```

---

### 4. Infeasibility Certificates

When an LP is unsatisfiable, `aries-lp` can generate a Farkas certificate that serves as a independently checkable proof of infeasibility:

```rust
use aries_lp::prelude::*;

let mut lp = Lp::new(OptimizationDirection::Maximize);
let x = lp.add_var(0.0, (0.0, f64::INFINITY));
let y = lp.add_var(0.0, (0.0, f64::INFINITY));

// x + y can not be both <= 5 and >= 10
lp.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Le, 5.0);
lp.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Ge, 10.0);

// Extract the certificate from the solve status
let certificate = match lp.solve().unwrap() {
    SolveStatus::Infeasible(InfeasibilityReason::Certified(cert)) => cert,
    _ => unreachable!("this problem is infeasible"),
};

// Verify the certificate against the current LP
assert!(lp.is_certificate_valid(&certificate));
```

### 5. Benchmark

To launch the benchmark, you first need to run the script *load_mps_files.sh* to get all the instances.

Then run the python file *bench.py*, the results will both be printed in the terminal and written in *results.txt*.

---

## License & Acknowledgments

This project is released under the [Apache License, Version 2.0](./LICENSE).

* Original `minilp` solver: Copyright (c) 2020 ztlpn ([GitHub](https://github.com/ztlpn/minilp)).
* Dynamic bound modifications, variable extensions, and feasibility checking by core contributors.
