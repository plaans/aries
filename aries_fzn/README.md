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


## Useful links
 - [Rust book](https://doc.rust-lang.org/stable/book/)
 - [Minizinc documentation](https://docs.minizinc.dev/en/stable/index.html)
 - [Minizinc playground](https://play.minizinc.dev/)
 - [Graphviz](https://graphviz.org/)