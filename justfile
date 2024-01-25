
set dotenv-load := true


ci: ci-up-solve ci-up-val

# Run planning tests for UP integration
ci-up-solve:
    python3 planning/unified/deps/unified-planning/up_test_cases/report.py aries

# Run validation tests for UP integration
ci-up-val:
    python3 planning/unified/deps/unified-planning/up_test_cases/report.py aries-val

# Solve a UP test case
up-solve problem:
    python3 planning/unified/scripts/cli.py {{problem}}

# Export a UP test case to a protobuf binary file (/tmp/problem.upp)
up-export problem:
    python3 planning/unified/scripts/cli.py {{problem}} -m dump