
# Aries Planning Engine (APE)

WIP: this in an ongoing effort to reimplement the Aries solver planning capabilities for PDDL.

The CLI tool currently proposes the following functionalities (most in heavy development)

 - parsing of PDDL files
 - validation of PDDL plans
 - optimization of PDDL plans
 - domain repair


## Domain repair

```sh
# Domain repair options.
cargo run  --bin aries-plan-engine   -- dom-repair --help
# Attempts to find fix to the (autmoatically infered) domain for this plan
cargo run  --bin aries-plan-engine   -- dom-repair planning/problems/pddl/tests/gripper.plan
```
