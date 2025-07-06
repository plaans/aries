
set dotenv-load := true


ci: ci-up-solve ci-up-val ci-ipc ci-warm-up



# Run planning tests for UP integration
ci-up-solve:
    python3 planning/unified/deps/unified-planning/up_test_cases/report.py aries -e up_aries_tests

# Run validation tests for UP integration
ci-up-val:
    python3 planning/unified/deps/unified-planning/up_test_cases/report.py aries-val -e up_aries_tests

# Run resolution tests on IPC problems
ci-ipc:
    ARIES_UP_ASSUME_REALS_ARE_INTS=true python3 ci/ipc.py

# Run tests for warm-starting
ci-warm-up:
    pytest planning/unified/plugin/test/test_warm_up.py -v -s

# Solve a UP test case
up-solve problem:
    python3 planning/unified/scripts/cli.py {{problem}}

# Export a UP test case to a protobuf binary file (/tmp/problem.upp)
up-export problem:
    python3 planning/unified/scripts/cli.py {{problem}} -m dump

# Solve specific IPC problems
ipc-solve +problem:
    ARIES_UP_ASSUME_REALS_ARE_INTS=true python3 ci/ipc.py {{problem}}

fzn-build:
    cargo build --release --bin aries_fzn
    cp target/release/aries_fzn aries_fzn/share

# Invoke minizinc, within the development environments (with aries compiled in release mode)
minizinc +args: fzn-build
    minizinc {{args}}

fzn-build-dbg:
    cargo build --profile ci --bin aries_fzn
    cp target/ci/aries_fzn aries_fzn/share

# Invoke minizinc, within the development environments (with aries compiled in ci mode)
minizinc-dbg +args: fzn-build-dbg
    minizinc {{args}}

# Shortcut command to run the samply profiler on a specific binary
samply bin +args:
    cargo build --profile perf --bin {{bin}}
    samply record target/perf/{{bin}} {{args}}



pddl-test-parser:
    #!/usr/bin/env bash
    set -e
    for f in `fd pb.pddl planning/problems/pddl/`; do
        cargo run --bin pddl-parser -- $f
    done
