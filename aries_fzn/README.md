# aries_fzn

aries_fzn is a Rust crate which aims to make Aries solver compatible with Minizinc. Minizinc problems are compiled to flatzinc which is the format supported by this crate.

Titouan Seraud - [titouan.seraud@laas.fr](mailto:titouan.seraud\@laas.fr) <!-- titouan.seraud@insa-toulouse.fr -->

<details>
<summary><b>Table of contents</b></summary>

- [Installation](#installation)
- [Usage](#usage)
- [Documentation](#documentation)
- [Useful links](#useful-links)
</details>

## Installation

### Compilation
Make sure you have installed [rustup](https://rustup.rs/). Compile the crate in release mode using the following command.
```bash
cargo build --release
```

### Minizinc setup
Make sure you have installed [minizinc](https://www.minizinc.org/).
Declare aries solver to minizinc by adding `share` directory to `MZN_SOLVER_PATH`.
```bash
export MZN_SOLVER_PATH=$PWD/aries_fzn/share
```

Minizinc should now detect aries via the file [aries.msc](share/aries.msc). You can check the solver is available using the following command.
```bash
minizinc --solvers
```


## Usage
Use `--solver` option to solve a problem with aries. For example, you can solve nqueens using the following command.
```bash
minizinc --solver aries aries_fzn/examples/nqueens.mzn -D n=8
```


## Documentation
The code is documented using Rust doc comments. Use cargo to create the documentation webpage.
```bash
cargo doc
```
You should now have access to the entry point [target/doc/aries_fzn/index.html](../target/doc/aries_fzn/index.html).

Other useful documentation can be found under the [doc](doc) directory.

## Adding a flatzinc constraint
Here are the steps to follow to add a new flatzinc constraint:
1. Update the redefinitions in [share/aries](share/aries) directory
2. Uncomment or write a new predicate in [predicates.fzn](meta/predicates.fzn)
3. Delete the files [constraint.rs](src/fzn/constraint/constraint.rs) and [mod.rs](src/fzn/constraint/builtins/mod.rs) in builtins
4. Run the script [constraints.py](meta/constraints.py) to generate code for the new constraint
5. Format the generated files with `cargo fmt`
6. Add a documentation comment on the generated struct
7. Implement the trait Encode for the new struct
8. If needed, add a new aries constraint in [aries/constraint](src/aries/constraint)
9. Add a new flatzinc test by creating a fzn-dzn pair in [tests/output](tests/output)
10. Add the new match case in parse_constraint_item in [parser.rs](src/fzn/parser.rs)
11. Verify the constraint is correctly implemented and tested with `cargo test`


## Useful links
 - [Rust book](https://doc.rust-lang.org/stable/book/)
 - [Minizinc documentation](https://docs.minizinc.dev/en/stable/index.html)
 - [Minizinc playground](https://play.minizinc.dev/)
 - [Graphviz](https://graphviz.org/)