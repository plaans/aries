Using the Aries timelines solver to solve the Beluga challenge proposed by Tuples.ai

### Usage

The project is in Rust so in order to install it you should have a working [rust installation](https://www.rust-lang.org/tools/install).
To compile it you should run:
```shell
cargo build --release --bin beluga
```
This will produce an executable binary `target/release/beluga` (target being at the root of this repository).

```shell
./beluga <path/to/instance>
```
There are instances in the instances/ directory.
