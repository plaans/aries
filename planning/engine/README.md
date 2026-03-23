
# Aries Planning Engine

WIP: this in an ongoing effort to reimplement the Aries solver planning capabilities for PDDL.




## Domain repair



```sh
# Domain repair options.
cargo run  --bin aries-plan-engine   -- dom-repair --help
# Attempts to find fix to the (autmoatically infered) domain for this plan
cargo run  --bin aries-plan-engine   -- dom-repair planning/problems/pddl/tests/gripper.plan
```
