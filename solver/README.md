

# Aries-solver library

This crate provides a library for general purpose constraint programming that is used as backend in several applications, most notably scheduling and automated planning.


The Aries project thus provides an innovative combinatorial solver that is built from the ground up by (1) mixing several techniques from constraint programming and automated reasoning, and (2) providing original representations and technologies relevant for automated planning:

- **Finite domain CSP**: at the core of a model are discrete variables with a finite integer domain denoting the set of possible values they can take.
- Literals represent expressions on the bounds of variables, for instance `(x <= 11)` or `(y > 10)`.
  This generalizes the literals in SAT solvers to non-boolean variables (whereas SMT or CSP solver typically maintain a correspondence table between such expressions and literals).
- Various **inference engines** are provided in the solver:
  - SAT based engine for disjunctive constraints (*clauses* in which at least one literal must be true), relying on unit clause propagation
  - Difference Logic engine (aka STN), for propagating temporal constraints or general difference constraints between two variables.
    The difference logic engine notably supports forward checking (or theory propagation) and native reasoning on optional variables.
  - General purpose CP engine for adding arbitrary constraints and the associated propagators (linear, max, no-overlap, ...).
- **Explanation and Clause learning** are supported by the various engines.
  When a conflict is detected during search, a new constraint will be inferred that prevents the solver of doing the same mistake.
- **Optional variables**: some variables can be optional: their presence in the solution will be determined by the value of a literal.
  This allows eager reasoning and constraint propagation by decoupling the presence literal and the domain of the variable.

While the aries solver library is built with automated planning problems in mind, it remains a general purpose solver that can be used for other combinatorial problems.

## Library modules

 - `prelude`: re-exports all common types and traits. In most cases `use aries_solver::prelude::*;` is all you need.
 - `core`: Low-level representation of variables, domains, literals. It also provides the implementation for the state (collection of domains) supporting backtracking and explanations.
 - `model`: Higher-level data structures and API to represent variables, expressions and their combination into constraint satisfaction problems.
 - `solver`: Implementation of combinatorial solvers with capabilities from SAT, SMT and CSP solvers. The solver provides an interface that accepts additional reasoners.
 - `reasoners`: Specialized reasoners that provide inference capabilities to main solver. It currently includes a reasoner for clauses, difference logic and one CP-like reasoner for classical propagators.
 - `backtrack`: Data structures for implementing trails that record event and allow undoing them. This crate provides a trail implementation meant for internal use in backtrackable structures and one that allows other entities to read the events that were pushed to the queue.
 - `collections`: Various collections that are focused on key-value collection where the key can be transformed into an array index.


## Example usage

The snippet below solves a tiny scheduling problem. We have a few tasks, each with a fixed duration, and some tasks must finish before others can start (*precedence* constraints). 
We look for start times that respect the precedences while finishing the whole project as early as possible (i.e. minimizing the *makespan*).

```rust
use aries_solver::prelude::*;

fn main() {
    // Duration of each task (here a tiny "morning routine"):
    //   0: wake up (5), 1: shower (10), 2: breakfast (15), 3: commute (30)
    let durations = [5, 10, 15, 30];

    // Precedences as (before, after) pairs: `before` must end before `after` starts.
    let precedences = [(0, 1), (0, 2), (1,3), (2, 3)];

    let mut model = Model::new();

    // For each task, create an interval with:
    //  - one decision variable denoting its start time, somewhere in [0, 100].
    //  - its fixed duration 
    let tasks: Vec<Interval> = durations.into_iter().map(|duration| {
        Interval::new_fixed_duration(
            model.new_variable(0,100),
            duration
        )
    }).collect();

    // Precedence constraints: end of `before` <= start of `after`.
    for &(before, after) in &precedences {
        model.enforce(leq(tasks[before].end, tasks[after].start));
    }

    // no two tasks can be carried out concurrently
    model.enforce(no_overlap(tasks.clone()));

    // The makespan is the time at which the last task finishes.
    // Minimizing it yields the shortest schedule.
    let makespan = model.new_variable(0, 100);
    model.enforce(eq_max(makespan, tasks.iter().map(|t| t.end)));

    // Search for the schedule that minimizes the makespan.
    let mut solver = Solver::new(model);
    match solver.minimize(makespan, SearchLimit::None) {
        Ok(Some((optimal_makespan, solution))) => {
            println!("Optimal makespan: {optimal_makespan}");
            for task in 0..durations.len() {
                println!("  task {task}: starts at {}", solution.eval(tasks[task].start).unwrap());
            }
            assert_eq!(optimal_makespan, 60, "Sanity check: the expected optimal makespan is 60");
        }
        Ok(None) => println!("No feasible schedule"),
        Err(_) => unreachable!("without a search limit the solver always returns a result"),
    }
}
```

The above example should print out a schedule like so (one of the two optimal schedules):

```text
Optimal makespan: 60
  task 0: starts at 0
  task 1: starts at 5
  task 2: starts at 15
  task 3: starts at 30
```

Several other examples are available in the `examples/` directory of this crate:

- [`sudoku.rs`](https://github.com/plaans/aries/blob/master/solver/examples/sudoku.rs): a Sudoku solver built on the `all_different` constraint.
- [`nqueens.rs`](https://github.com/plaans/aries/blob/master/solver/examples/nqueens.rs): the N-Queens problem, using `!=` constraints.
- [`knapsack.rs`](https://github.com/plaans/aries/blob/master/solver/examples/knapsack.rs): the 0/1 knapsack problem, maximizing a linear objective.
- [`ilp.rs`](https://github.com/plaans/aries/blob/master/solver/examples/ilp.rs): a generic integer linear program solver.
- [`sat.rs`](https://github.com/plaans/aries/blob/master/solver/examples/sat.rs): a SAT solver working on clauses (disjunctions of literals).
- [`orienteering.rs`](https://github.com/plaans/aries/blob/master/solver/examples/orienteering.rs): the orienteering routing problem.

Examples can be run with cargo, e.g., `cargo run --example sudoku`

Note that the examples here are intended as introductory examples to showcase the API and do not leverage all features of the solvers and most likely leave some performance on the table.


## References

   - Arthur Bit-Monnot. *Enhancing Hybrid CP-SAT Search for Disjunctive Scheduling* -- ECAI 2023 [(link)](https://hal.science/hal-04174800)
     - describes the core solver design, based on lazy clause generation, and reports on performance on disjunctive scheduling problems (Jobshop and Openshop)
   - Arthur Bit-Monnot. *Revisiting Optional Variables in Lazy Clause Generation Solvers for Flexible Scheduling* -- CP 2026 [(link)](https://doi.org/10.4230/LIPIcs.CP.2026.7)
     - Describes the support for optional variables in the solver and reports on the performance on flexible scheduling problems (Flexible jobshop, and variants with transport times and maximum time-lag)
     - Repository with benchmark instances and code: [https://github.com/plaans/flexible-jobshop-benchmarks](https://github.com/plaans/flexible-jobshop-benchmarks)

The scheduling solver for the two references above in the [`examples/scheduling`](https://github.com/plaans/aries/tree/master/examples/scheduling) directory of the [`plaans/aries`](https://github.com/plaans/aries) main repository.
