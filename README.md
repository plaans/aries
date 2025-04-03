# aries-fzn

aries-fzn is a Rust crate which aims to make Aries solver compatible with Minizinc. Minizinc problems are compiled to flatzinc which is the format supported by this crate.

Titouan Seraud - [titouan.seraud@laas.fr](titouan.seraud@laas.fr)


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
export MZN_SOLVER_PATH=$PWD/share
```

Aries solver should now be available. For 
```bash
minizinc --solvers
```


## Usage

To solve a problem using aries use `--solver` option. For example you can solve nqueens using the following command.
```bash
minizinc --solver aries examples/nqueens.mzn -D n=8
```
