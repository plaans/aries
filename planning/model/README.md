
# PlanX: Aries Plan Exchange library

A rust library for manipulating planning problems.

WARNING: this library is still in early stages and while we already find is useful for many purposes, it will contain some bugs in the less tested areas. API is also subject to important changes.


The library has two main components:

 - a generic model for planning problem, notably aiming for good support for temporal and numeric domain and complex metrics.
 - an extensive PDDL parser: while the planning model is agnostic of PDDL or any other input language, we waim to provide first class support for importing existing model


 PDDL features supported:

  - [X] classical planning
  - [X] durative actions (PDDL2.1)
  - [X] timed initial literals
  - [X] numeric fluents (PDDL2.1): add/increase
  - [X] constraints and preferences (PDDL3.0)
  - [X] multi-valued state variables
  - [X] conditional effects, quantification (forall, exists)
  - [X] HDDL: tasks, methods, ...
  - [ ] Continuous change
  - [ ] axioms (no planned support)

We typically aim for good and unambiguous error messages.


### Usage

Usage is primarily intended as a library, but we provide a standalone (CLI) parser that just prints out the parsed model and any error found.

The command-line parser can be found in the `pddl-parser` executable of the crate (source: `src/bin/pddl_parser.rs`)

#### Example error output (looks better with colors)


```
Error: error: has type `rover` but type `waypoint` was expected: ?r
   --> rovers/domain-pp01-err-rate-0-1.pddl:156:12
    |
 27 |     (visible ?w - waypoint ?p - waypoint)
    |     ------------------------------------- fluent declaration
...
146 | (:action communicate_rock_data
    |          --------------------- when parsing action
147 |     :parameters (?r - rover
148 |         ?l - lander
149 |         ?p - waypoint
150 |         ?x - waypoint
151 |         ?y - waypoint)
152 |     :precondition (and
153 |         (at ?r ?x)
154 |         (at_lander ?l ?y)
155 |         (have_rock_analysis ?r ?p)
156 |         (visible ?r ?y)
    |         ---------^^----
    |         |        |
    |         |        has type `rover` but type `waypoint` was expected
    |         when parsing expression
157 |         (available ?r)
158 |         (channel_free ?l))
159 |     :effect (and
160 |         (channel_free ?l)
161 |         (communicated_rock_data ?p)
162 |         (available ?r)))
    |
```
