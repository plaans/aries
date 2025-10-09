
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





# Parses all instance-1.pddl PDDL files in planning/ext/pddl except for the few known to be unsupported.
pddl-parse-all filter="d":
    #!/usr/bin/env bash
    set -e  # stop on first error
    cargo build --profile ci --bin pddl-parser
    # ignore the following domains
    #  - from 1998 IPC (non stabilized syntax)
    #  - with derived predicates
    #  - temporal machine shop (TMS, IPC 2011 and 2014) that contains an object declared twice with two distinct type (valid PDDL but unsupported)
    for f in `fd instance-1.[hp]ddl planning/ext/pddl/ | grep -v "1998" | grep {{filter}} |  grep -v derived | grep -v temporal-machine-shop `; do
        echo $f
        target/ci/pddl-parser  $f > /dev/null
    done
