//! Black-box tests for incremental model additions (constraints and variables).
//!
//! Random LPs are solved in two ways and the results compared:
//!
//! - *incrementally*: a single `Lp` is extended (constraints and/or variables are added)
//!   and re-solved after each extension;
//! - *from scratch*: a fresh `Lp` containing exactly the same model is solved once.
//!
//! Both must agree on the solve status. When both find an optimum, the objective values must
//! match, and each solution must satisfy all constraints and variable bounds (variable values
//! are not compared directly: an LP may have several optimal assignments).
//!
//! Note that `Lp` builds its internal solver lazily, on the first solve. The incremental path
//! is therefore only exercised by additions that happen *after* a first solve.

use aries_lp::prelude::*;
use rand::{Rng, SeedableRng, rngs::SmallRng, seq::IteratorRandom};

/// Tolerance for floating point comparisons.
///
/// Models only use small integer coefficients, so solutions should be nearly exact
/// and any simplex noise stays far below this.
const TOL: f64 = 1e-6;

/// Number of random LPs tested by each test.
const NB_SEEDS: u64 = 1000;

/// Specification of a variable: its coefficient in the objective and its bounds.
struct VarSpec {
    obj_coeff: f64,
    lb: f64,
    ub: f64,
}

/// A linear program description: variables and constraints, in construction order.
///
/// Invariant: all LPs built from a `Model` add the variables in the same order, so that
/// variable handles (plain indices) are interchangeable between builds.
struct Model {
    direction: OptimizationDirection,
    vars: Vec<VarSpec>,
    constraints: Vec<Constraint>,
}

/// Outcome of a solve, reduced from `SolveStatus` / `SolverFailure`.
#[derive(Debug, PartialEq, Eq)]
enum Status {
    Optimal,
    Infeasible,
    Unbounded,
    Unstable,
}

/// Adds to `lp` the constraint `c`.
///
/// The canonical form is used, in which the right-hand side is already folded
/// into the constant of the expression, hence the comparison against `0.0`.
fn add_to_lp(lp: &mut Lp, c: &Constraint) {
    let canonical = c.canonical();
    lp.add_constraint(canonical.expr().clone(), canonical.comp_op(), 0.0);
}

/// Builds a fresh LP with all the model's variables and its `nb_constraints` first constraints.
fn build_lp(model: &Model, nb_constraints: usize) -> Lp {
    let mut lp = Lp::new(model.direction);
    for (i, var) in model.vars.iter().enumerate() {
        let handle = lp.add_var(var.obj_coeff, (var.lb, var.ub));
        assert_eq!(
            handle.idx(),
            i,
            "variables must be added in the same order in all builds"
        );
    }
    for c in &model.constraints[..nb_constraints] {
        add_to_lp(&mut lp, c);
    }
    lp
}

/// Solves `lp` and extracts its status, plus the objective and variable values if an optimum was found.
fn run_solve(lp: &mut Lp, nb_vars: usize) -> (Status, Option<f64>, Option<Vec<f64>>) {
    match lp.solve() {
        Ok(SolveStatus::Optimal(solution)) => {
            let mut values = vec![f64::NAN; nb_vars];
            for (var, val) in &solution {
                values[var.idx()] = *val;
            }
            (Status::Optimal, Some(solution.objective()), Some(values))
        }
        Ok(SolveStatus::Infeasible(_)) => (Status::Infeasible, None, None),
        Ok(SolveStatus::Unbounded) => (Status::Unbounded, None, None),
        Err(SolverFailure::Unstable) => (Status::Unstable, None, None),
        Err(SolverFailure::Timeout) => unreachable!("no deadline was set"),
    }
}

/// Builds and solves the model from scratch: all variables, `nb_constraints` first constraints.
fn solve_from_scratch(model: &Model, nb_constraints: usize) -> (Status, Option<f64>, Option<Vec<f64>>) {
    let mut lp = build_lp(model, nb_constraints);
    run_solve(&mut lp, model.vars.len())
}

/// Compares the incremental solve of `lp` against a from-scratch solve of the same model,
/// and checks the validity of the incremental solution.
fn compare_with_from_scratch(lp: &mut Lp, model: &Model, nb_constraints: usize, seed: u64, context: &str) {
    let (status, objective, values) = run_solve(lp, model.vars.len());
    let (ref_status, ref_objective, _) = solve_from_scratch(model, nb_constraints);

    assert_eq!(status, ref_status, "seed {seed}: status mismatch in {context}");

    if status == Status::Optimal {
        let objective = objective.unwrap();
        assert!(
            (objective - ref_objective.unwrap()).abs() <= TOL,
            "seed {seed}: objective mismatch in {context}: {objective} vs from scratch",
        );
        assert_solution_valid(model, nb_constraints, objective, &values.unwrap(), seed, context);
    }
}

/// Checks that `values` are within the variable bounds, satisfy the model's first
/// `nb_constraints` constraints, and yield the reported objective.
fn assert_solution_valid(
    model: &Model,
    nb_constraints: usize,
    objective: f64,
    values: &[f64],
    seed: u64,
    context: &str,
) {
    assert_eq!(
        values.len(),
        model.vars.len(),
        "seed {seed}: wrong number of values in {context}"
    );
    for (i, (var, value)) in model.vars.iter().zip(values).enumerate() {
        assert!(
            var.lb - TOL <= *value && *value <= var.ub + TOL,
            "seed {seed}: variable {i} out of bounds in {context}: {value} not in [{}, {}]",
            var.lb,
            var.ub
        );
    }
    for constraint in &model.constraints[..nb_constraints] {
        assert!(
            is_satisfied(constraint, values),
            "seed {seed}: unsatisfied constraint in {context}: {constraint:?}",
        );
    }
    let computed: f64 = model
        .vars
        .iter()
        .zip(values)
        .map(|(var, value)| var.obj_coeff * value)
        .sum();
    assert!(
        (computed - objective).abs() <= TOL,
        "seed {seed}: reported objective {objective} does not match computed {computed} in {context}",
    );
}

/// Evaluates the canonical expression of the constraint at `values` and checks the comparison to zero.
fn is_satisfied(constraint: &Constraint, values: &[f64]) -> bool {
    let canonical = constraint.canonical();
    let expr = canonical.expr();
    let lhs: f64 = expr.iter().map(|(var, factor)| factor * values[var.idx()]).sum();
    let lhs = lhs + expr.constant();
    match canonical.comp_op() {
        LEQ => lhs <= TOL,
        GEQ => lhs >= -TOL,
        EQ => lhs.abs() <= TOL,
    }
}

/// A random variable: objective coefficient in [-5, 5], lower bound in [-10, 0], upper bound in [1, 10].
///
/// Bounds are finite, so unbounded LPs can never be generated.
fn random_var(rng: &mut SmallRng) -> VarSpec {
    VarSpec {
        obj_coeff: rng.gen_range(-5, 6) as f64,
        lb: rng.gen_range(-10, 1) as f64,
        ub: rng.gen_range(1, 11) as f64,
    }
}

/// A random constraint over `vars`: a non-empty subset of the variables with non-zero integer
/// factors, plus a random constant, comparison operator and right-hand side (all small integers).
fn random_constraint(rng: &mut SmallRng, vars: &[Variable]) -> Constraint {
    let nb_terms = rng.gen_range(1, vars.len() as i32 + 1) as usize;
    let mut expr = LinExpr::zero();
    expr += rng.gen_range(-10, 11) as f64;
    for var in (0..vars.len()).choose_multiple(rng, nb_terms) {
        let factor = rng.gen_range(1, 6) * if rng.gen_bool(0.5) { 1 } else { -1 };
        expr += vars[var] * (factor as f64);
    }
    let op = match rng.gen_range(0, 3) {
        0 => LEQ,
        1 => GEQ,
        _ => EQ,
    };
    let rhs = rng.gen_range(-10, 11) as f64;
    expr.into_constraint(op, rhs)
}

/// Constraints are added in random batches to an LP that is re-solved after each batch.
///
/// At least two batches are guaranteed, so constraints are always added to an LP that has
/// already been solved (the incremental path).
#[test]
fn incremental_constraints_match_from_scratch() {
    for seed in 0..NB_SEEDS {
        let mut rng = SmallRng::seed_from_u64(seed);
        let direction = if rng.gen_bool(0.5) {
            OptimizationDirection::Maximize
        } else {
            OptimizationDirection::Minimize
        };
        let vars: Vec<VarSpec> = (0..rng.gen_range(2, 8)).map(|_| random_var(&mut rng)).collect();
        let nb_constraints = rng.gen_range(5, 16) as usize;

        // The incremental build creates the variables, which provide the handles used in the constraints.
        let mut lp = Lp::new(direction);
        let handles: Vec<Variable> = vars.iter().map(|v| lp.add_var(v.obj_coeff, (v.lb, v.ub))).collect();
        let constraints: Vec<Constraint> = (0..nb_constraints)
            .map(|_| random_constraint(&mut rng, &handles))
            .collect();
        let model = Model {
            direction,
            vars,
            constraints,
        };

        let mut added = 0;
        while added < nb_constraints {
            let batch = rng.gen_range(1, 5).min((nb_constraints - added) as i32) as usize;
            for c in &model.constraints[added..added + batch] {
                add_to_lp(&mut lp, c);
            }
            added += batch;

            compare_with_from_scratch(&mut lp, &model, added, seed, &format!("after {added} constraints"));
        }
    }
}

/// Variables and constraints are added by rounds to an LP that is re-solved after each round.
///
/// Variables are added after constraints exist, exercising the internal re-indexing of
/// variables (including slack variables) on the incremental path.
#[test]
fn incremental_variables_match_from_scratch() {
    for seed in 0..NB_SEEDS {
        let mut rng = SmallRng::seed_from_u64(seed);
        let direction = if rng.gen_bool(0.5) {
            OptimizationDirection::Maximize
        } else {
            OptimizationDirection::Minimize
        };

        // Initial model: 2 to 4 variables and 1 to 3 constraints over them.
        let mut model = Model {
            direction,
            vars: (0..rng.gen_range(2, 5)).map(|_| random_var(&mut rng)).collect(),
            constraints: vec![],
        };
        let mut lp = Lp::new(direction);
        let mut handles: Vec<Variable> = model
            .vars
            .iter()
            .map(|v| lp.add_var(v.obj_coeff, (v.lb, v.ub)))
            .collect();
        for _ in 0..rng.gen_range(1, 4) {
            let c = random_constraint(&mut rng, &handles);
            add_to_lp(&mut lp, &c);
            model.constraints.push(c);
        }
        compare_with_from_scratch(&mut lp, &model, model.constraints.len(), seed, "initial model");

        // Each round adds 1-2 variables and 1-3 constraints over all variables so far.
        for round in 1..=rng.gen_range(1, 4) {
            for _ in 0..rng.gen_range(1, 3) {
                let var = random_var(&mut rng);
                handles.push(lp.add_var(var.obj_coeff, (var.lb, var.ub)));
                model.vars.push(var);
            }
            for _ in 0..rng.gen_range(1, 4) {
                let c = random_constraint(&mut rng, &handles);
                add_to_lp(&mut lp, &c);
                model.constraints.push(c);
            }
            compare_with_from_scratch(
                &mut lp,
                &model,
                model.constraints.len(),
                seed,
                &format!("round {round}"),
            );
        }
    }
}
