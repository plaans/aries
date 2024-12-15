
set dotenv-load := true


ci: ci-up-solve ci-up-val ci-ipc

test: test-i32 test-i64 test-i128 test-i64_i128

# Run planning tests for UP integration
ci-up-solve:
    python3 planning/unified/deps/unified-planning/up_test_cases/report.py aries -e up_aries_tests

# Run validation tests for UP integration
ci-up-val:
    python3 planning/unified/deps/unified-planning/up_test_cases/report.py aries-val -e up_aries_tests

# Run resolution tests on IPC problems
ci-ipc:
    ARIES_UP_ASSUME_REALS_ARE_INTS=true python3 ci/ipc.py

# Solve a UP test case
up-solve problem:
    python3 planning/unified/scripts/cli.py {{problem}}

# Export a UP test case to a protobuf binary file (/tmp/problem.upp)
up-export problem:
    python3 planning/unified/scripts/cli.py {{problem}} -m dump

# Solve specific IPC problems
ipc-solve +problem:
    ARIES_UP_ASSUME_REALS_ARE_INTS=true python3 ci/ipc.py {{problem}}

# Run tests using i32 as IntCst
test-i32:
    cargo test

# Run tests using i64 as IntCst
test-i64:
    cargo test --features i64

# Run tests using i128 as IntCst
test-i128:
    cargo test --features i128

# Verify that aries doesn't build with both i128 and i64 features
test-i64_i128:
    @echo "checking i128 and i64 can't be enabled simultaneously"
    if cargo test --features i128,i64; then echo "expected cargo test to fail when both i64 and i128 are enabled"; exit 1; fi
    @echo "ok"