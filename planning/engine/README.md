
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
