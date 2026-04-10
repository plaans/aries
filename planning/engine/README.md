
# Aries Planning Engine (APE)

Disclaimer this in an ongoing effort to reimplement the Aries solver planning capabilities for PDDL and different functionalities have different maturity levels.
In particular, elements marked as WIP are not feature complete and should not be relied upon.

The CLI tool currently proposes the following functionalities:

 - `parse`: parsing of PDDL problem files
 - `parse-domain`: parsing of lone PDDL domain file
 - `validate` (WIP): validate a PDDL plan
 - `plan-optimization` (WIP): optimize a given PDDL plans, specifying the target metrics and allowed relaxations
 - `dom-repair`: Propose fixes of a broken PDDL domain so that a target plan is valid.

 ## Installation
 
The usage section assumes that you have the `ape` executable on your path. It can be installed globally from the latest version in the git repository (but requires a [rust toolchain](https://rust-lang.org/tools/install/)).

```sh
# Install 
cargo install --bin ape --git https://github.com/plaans/aries aries-plan-engine
```

To install from a locally checked-out version of aries:

```sh
# Install `ape` globally, assuming that you are in the same directory as this file (planning/engine/).
cargo install --bin ape --path .
```

For development, you can simply replace the `ape` command by `cargo run [--release] --bin ape --` which will compile `ape` and then run it with the arguments provided after the finals `--`.
Omitting the `--release` flag will produce non-optimized builds that would be substantially faster to compile (but much slower for execution.)

## Usage

```sh
# List available commands 
ape --help

# Print help for a particular command (here `validate`)
ape validate --help
```


## Naming conventions for PDDL problems and domains

Ape relies on naming conventions to find the domain associated to a problem files (which avoids having to specify it manually on the command line).

Given a problem file, `ape` would consider candidate domain files in the same or parent directory, with a set of possible name depending on the problem filename:

- `*.pddl` -> `domain.pddl`
- `XXXXX.YY.pb.pddl` -> `XXXXX.dom.pddl`
- `XXXXX.pb.pddl` -> `XXXXX-domain.pddl`
- `instance-NN.pddl` -> `domain-NN.pddl`

These are implemented in the [find_domain_of](https://github.com/plaans/aries/blob/master/planning/planx/src/pddl/find_file.rs) function, which may serve as a reference.

This can be used on the command line with the `find-domain` command, that will print the domain file on the standard output.
This can be useful when integration with other tools, e.g., 
```sh
my-planner --domain `ape find-domain ${PROBLEM_FILE}` --problem ${PROBLEM_FILE}
```
It is notably used in the [`zed-pddl`](https://github.com/arbimo/zed-pddl) extension for Zed.

Note that at this point, the discovery is purely based on the filenames, with no attempt to validate that the domain name matches the one declared in the problem.



## Plan Optimization

The `optimize-plan` command allow specify objectives:

 - `makespan`: minimizes the end time of the last action
 - `plan-length`: minimizes the number of actions

It also allow to specify the allowed relaxations (what is allowed to change in the plan):

 - `action-presence`: allow removing actions
 - `start-time`: allow changing the start time of actions

### Example plan

The file [examples/gripper/plan](examples/gripper/plan) contains the following plan:

```
(pick ball1 rooma left)
(pick ball2 rooma right)
(move rooma roomb)
(move roomb rooma)
(move rooma roomb)
(drop ball1 roomb left)
(drop ball2 roomb right)
(move roomb rooma)
(pick ball3 rooma left)
(pick ball4 rooma right)
(move rooma roomb)
(drop ball3 roomb left)
(drop ball4 roomb right)
```

### No relaxation -> same plan

Running the `optimize-plan` command with no relaxation will always give the same plan and will succeed if the plan is valid:

```
> ape optimize-plan examples/gripper/plan -o plan-length
...
==== Plan (objective: 13) =====
   0: (pick left ball1 rooma) [0]
   1: (pick right ball2 rooma) [0]
   2: (move rooma roomb) [0]
   3: (move roomb rooma) [0]
   4: (move rooma roomb) [0]
   5: (drop left ball1 roomb) [0]
   6: (drop right ball2 roomb) [0]
   7: (move roomb rooma) [0]
   8: (pick left ball3 rooma) [0]
   9: (pick right ball4 rooma) [0]
  10: (move rooma roomb) [0]
  11: (drop left ball3 roomb) [0]
  12: (drop right ball4 roomb) [0]
```

Notice that here, the plan is unchanged. In case of a sequential plan, the start time of each action corresponds to its position in the original plan.


### Action presence relaxation

Relaxing action presence (and optimize plan length) will yield a new plan with the third and fourth actions removed:

```
> ape optimize-plan examples/gripper/plan -o plan-length -r action-presence
...
==== Plan (objective: 11) =====
   0: (pick left ball1 rooma) [0]
   1: (pick right ball2 rooma) [0]
   4: (move rooma roomb) [0]
   5: (drop left ball1 roomb) [0]
   6: (drop right ball2 roomb) [0]
   7: (move roomb rooma) [0]
   8: (pick left ball3 rooma) [0]
   9: (pick right ball4 rooma) [0]
  10: (move rooma roomb) [0]
  11: (drop left ball3 roomb) [0]
  12: (drop right ball4 roomb) [0]
```

Note that the timing of actions does not change, and two unoccopied time-stamps remain (2 and 3)


### Action timing relaxation

Relaxing action start times (and optimizing makespan) will give a plan where actions are freely allowed to move around and packed so has to minimize the total time:


```
> ape optimize-plan examples/gripper/plan -o makespan -r start-time
...
==== Plan (objective: 8) =====
   0: (pick right ball2 rooma) [0]
   0: (pick left ball3 rooma) [0]
   1: (move rooma roomb) [0]
   2: (drop right ball2 roomb) [0]
   2: (drop left ball3 roomb) [0]
   3: (move roomb rooma) [0]
   4: (pick left ball1 rooma) [0]
   4: (pick right ball4 rooma) [0]
   5: (move rooma roomb) [0]
   6: (drop left ball1 roomb) [0]
   6: (drop right ball4 roomb) [0]
   7: (move roomb rooma) [0]
   8: (move rooma roomb) [0]
```

Here the pick actions are performed in parallel when possible.
Note that this goes beyond a partial order extraction: the two useless move actions have been repositioned at the end of the plan (i.e. changing some causal links in the plan).
However, those actions were not removed: without the `action-presence` relaxation, the solver is not allowed to remove them.


### Combining relaxations

Relaxation can of course be combined, e.g., to allow both removing actions and moving them around:

```
> ape optimize-plan examples/gripper/plan -o makespan -r start-time -r action-presence
...
==== Plan (objective: 6) =====
   0: (pick right ball2 rooma) [0]
   0: (pick left ball3 rooma) [0]
   1: (move rooma roomb) [0]
   2: (drop right ball2 roomb) [0]
   2: (drop left ball3 roomb) [0]
   3: (move roomb rooma) [0]
   4: (pick left ball1 rooma) [0]
   4: (pick right ball4 rooma) [0]
   5: (move rooma roomb) [0]
   6: (drop left ball1 roomb) [0]
   6: (drop right ball4 roomb) [0]
```
