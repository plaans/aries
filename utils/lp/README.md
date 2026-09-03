# aries-lp

**TO BE CHANGED**

[![Crates.io](https://img.shields.io/crates/v/aries-lp.svg)](https://crates.io/crates/aries-lp)
[![Documentation](https://docs.rs/aries-lp/badge.svg)](https://docs.rs/aries-lp/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](./LICENSE)

A fast, pure-Rust linear programming solver library with support for dynamic bound modifications, incremental variable/constraint additions, and feasibility checking.

This crate is a fork and enhancement of [`minilp`](https://crates.io/crates/minilp) originally written by [ztlpn](https://github.com/ztlpn/minilp).

[Linear programming](https://en.wikipedia.org/wiki/Linear_programming) is a technique for finding the minimum (or maximum) of a linear function of a set of continuous variables subject to linear equality and inequality constraints.

---

## What's New in this Fork?

Compared to standard `minilp`, this crate introduces:

* **Dynamic Bound Updates**: Modify variable lower and upper bounds on existing problems (`Problem::set_bound`) and solutions (`Solution::set_bound_var`).
* **Dynamic Variable Addition**: Add new variables to existing problem structures or feasibility checkers on the fly (`FeasibilityChecker::add_variable`).
* **Dedicated Feasibility Checker**: A specialized `FeasibilityChecker` wrapper to rapidly verify feasibility under incremental bound modifications and additions without requiring full re-optimization.
* **Infeasibility Certificates**: Generate and validate mathematical certificates of unsatisfiability (`Error::InfeasibleWithCertificate` & `Problem::is_certificate_valid`).

---

## Features

* **Pure Rust** implementation.
* Able to solve problems with hundreds of thousands of variables and constraints.
* **Incremental**: Add constraints or modify bounds on an existing solution without solving from scratch.
* **Feasibility Engine**: Dedicated checking mechanism for dynamic constraint satisfaction problem solving.
* **MPS Format Support**: Problems can be defined via code API or parsed from an [MPS](https://en.wikipedia.org/wiki/MPS_(format)) file.

> **Warning**: Like `minilp`, this library uses floating-point simplex pivoting operations. Harder problems may cycle, suffer numerical instability, or panic. Please report bugs and contribute code! Certificates are intended to help you detect false unsatsfiability however, we do not provide certificates for satisfiability.

---

## Usage & Examples

Add this to your `Cargo.toml`:

```toml
[dependencies]
aries-lp = "0.1.0"  # Replace with your actual crate name & version

```

### 1. Basic Problem Solving

```rust
use aries_lp::{Problem, OptimizationDirection, ComparisonOp};

// Maximize x + 2 * y subject to x >= 0, 0 <= y <= 3
let mut problem = Problem::new(OptimizationDirection::Maximize);
let x = problem.add_var(1.0, (0.0, f64::INFINITY));
let y = problem.add_var(2.0, (0.0, 3.0));

// Constraints: x + y <= 4 and 2 * x + y >= 2
problem.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Le, 4.0);
problem.add_constraint([(x, 2.0), (y, 1.0)], ComparisonOp::Ge, 2.0);

// Optimal value is 7, achieved at x = 1 and y = 3
let solution = problem.solve().unwrap();
assert_eq!(solution.objective(), 7.0);
assert_eq!(solution[x], 1.0);
assert_eq!(solution[y], 3.0);

```

---

### 2. Dynamic Bound Modifications & Incremental Solving

You can update bounds on active solutions without re-building the problem from scratch:

```rust
use aries_lp::{Problem, OptimizationDirection, ComparisonOp, Bound};

let mut problem = Problem::new(OptimizationDirection::Maximize);
let x = problem.add_var(1.0, (0.0, 3.0));
let y = problem.add_var(2.0, (0.0, 3.0));
problem.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Le, 4.0);

let solution = problem.solve().unwrap();
assert_eq!(solution.objective(), 7.0); // x = 1.0, y = 3.0

// Restrict upper bound of x to 0.5
let updated_solution = solution.set_bound_var(x, 0.5, Bound::Upper).unwrap();
assert_eq!(updated_solution[x], 0.5);
assert_eq!(updated_solution[y], 3.0);
assert_eq!(updated_solution.objective(), 6.5);

```

---

### 3. Using `FeasibilityChecker` for Fast Feasibility Verification

`FeasibilityChecker` is designed for interactive algorithms (e.g. branch-and-bound or SAT/SMT-like engines) that need to push/pop bounds or add variables dynamically while maintaining feasibility.

```rust
use aries_lp::{Problem, OptimizationDirection, ComparisonOp, Bound, Variable};

let mut problem = Problem::new(OptimizationDirection::Maximize);
let v1 = problem.add_var(1.0, (0.0, 3.0));
let v2 = problem.add_var(2.0, (0.0, 3.0));
problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);

let mut feas_checker = problem.create_feasibility_checker().unwrap();

// Check feasibility of initial state
assert!(feas_checker.check_feasibility().is_ok());

// Add a new variable on the fly
let v3_idx = feas_checker.add_variable(0.0, -10.0, 10.0).unwrap();
let v3 = Variable(v3_idx);

// Add constraint linking existing variables and the new variable
feas_checker.add_constraint([(v1, 1.0), (v3, 1.0)], ComparisonOp::Ge, 2.0).unwrap();

// Dynamically tighten lower bound on v2
feas_checker.set_bound(v2, &Bound::Lower, 2.0).unwrap();

// Verify whether the system remains feasible
if feas_checker.check_feasibility().is_ok() {
    println!("Problem state is feasible!");
}

```

---

### 4. Infeasibility Certificates

When a problem or feasibility check fails due to unsatisfiable constraints, `aries-lp` can generate an infeasibility certificate:

```rust
use aries_lp::{Problem, OptimizationDirection, ComparisonOp, Error};

let mut problem = Problem::new(OptimizationDirection::Maximize);
let x1 = problem.add_var(0.0, (0.0, f64::INFINITY));
let x2 = problem.add_var(0.0, (0.0, f64::INFINITY));

problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Le, 5.0);
problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Ge, 10.0);

match problem.solve() {
    Err(Error::InfeasibleWithCertificate(cert)) => {
        // Verify certificate validity against the original problem definition
        assert!(problem.is_certificate_valid(&cert));
        println!("Generated valid infeasibility certificate: {:?}", cert);
    }
    _ => unreachable!(),
}

```

---

## License & Acknowledgments

This project is released under the [Apache License, Version 2.0](./LICENSE).

* Original `minilp` solver: Copyright (c) 2020 ztlpn ([GitHub](https://www.google.com/url?sa=E&source=gmail&q=https://github.com/ztlpn/minilp)).
* Dynamic bound modifications, variable extensions, and `FeasibilityChecker` additions by core contributors.