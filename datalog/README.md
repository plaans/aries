# A (very) minimal datalog inference engine

This crate aims to provide a very minimal datalog engine. 
While the engine should be very general, its main purpose is to be used for grounding planning problems (in the aries family of planners).

The objective of the crate derives from this use-case:

 - dynamic program creation: the rules are not known at compile time but discovered when parsing a planning problem file. This makes most of rust datalog engines not usable as they are design for rules known at compile time (which enables many compiler optimization)
 - minimal dependencies and no-async to make it easily embeddable 
 - minimal and low-level API, using integers ID to represent symbols and variables and interior mutability for updating derived facts.
 - reasonable performance, driven by planning use cases: this does not aim at being the fastest, but should be fast enough so that grounding is not a bottleneck on the considered planning benchmarks.

 This crate was written as I did not find any existing one meeting all those requirements (the first two ones being the hardest to satisfy in conjunction in the current ecosystem).
 We would happily accept improvement if they do not negatively affect the main use case.

 ## Example

Below is an example for encoding the usual "finding ancestors" datalog program.
An example targeting for grounding a planning problem is available in the `examples/`.


```prolog
parent(x1, x2).
parent(x2, x3).
parent(x3, x4).

ancestor(?x, ?y) :- parent(?x, ?y).
ancestor(?x, ?y) :- ancestor(?x, ?y), parent(?y, ?z).
```

The task of inferring all ancestors can be encoded as follows:

```rust
use aries_datalog::*;
// create a program that will contain the predicates, facts and rules.
let mut prog = Program::new();

let parent = prog.new_predicate(2);
parent.add([1, 2]); // parent(x1, x2).
parent.add([2, 3]);
parent.add([3, 4]);

let ancestor = prog.new_predicate(2);

// a parent is an ancestor
// ancestor(?x, ?y) :- parent(?x, ?y).
prog.add_rule(Rule::new(
    ancestor.apply([Arg::Var(0), Arg::Var(1)]),
    [
        parent.apply([Arg::Var(0), Arg::Var(1)]),
    ]
));

// the parent of an ancestor is an ancestor
// ancestor(?x, ?y) :- ancestor(?x, ?y), parent(?y, ?z).
prog.add_rule(Rule::new(
    ancestor.apply([Arg::Var(0), Arg::Var(2)]),
    [
        ancestor.apply([Arg::Var(0), Arg::Var(1)]),
        parent.apply([Arg::Var(1), Arg::Var(2)]),
    ]
));

// run inference
prog.run();

// as a result of inference, the `ancestor` table has been populated with the result of all inferences.
assert_eq!(
    ancestor.extract().rows_sized(),
    &[
        [1, 2],
        [1, 3],
        [1, 4],
        [2, 3],
        [2, 4],
        [3, 4]
    ]
);
```
