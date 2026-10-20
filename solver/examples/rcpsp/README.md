Minimal solver for the multi-mode resource-constrained project scheduling problem (MRCPSP),
built on the `aries-solver` library.

The solver only provides a minimalist model: it creates one optional variable per (job, mode) pair and posts
precedence, mode-selection, renewable-resource (`Cumulative`) and nonrenewable-resource
constraints. Search and inference are left to the default behavior of the Aries solver.

**Disclaimer:** the parsing and problem encoding code was generated with an LLM. It is primarily intended for testing the cumulative constraint and there are several things that should be improved to reach optimal performance.

### Usage

```shell
cargo run --release --example rcpsp -- <path/to/instance.mm>
```

Options include `--timeout/-t` (maximum runtime in seconds) and `--expected-makespan`
(exits with an error if the optimal makespan differs from the given value).

### Instances

Instances follow the PSPLIB multi-mode format ([PSPLIB](https://www.om-db.wi.tum.de/psplib/),
`j10` set). A few are bundled in the `instances/` folder, together with `tiny.mm`, a
handcrafted instance with a known optimal makespan of 10.

More instances (used for testing in CI) are availavle in the [aries-benchmarks](https://github.com/plaans/aries-benchmarks) repository.
