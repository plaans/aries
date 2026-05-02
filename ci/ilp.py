#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the ilp solver finds the optimal solution for a few instances.

import os
import subprocess
import shlex

res = os.system("cargo build --profile ci --bin aries-ilp")
if res != 0:
    exit(1)
solver = "target/ci/aries-ilp"

solver_cmd = solver + " {instance} {no_lprelax} {expected}"

instances = [
    ("simple", "examples/ilp/instances/lp/sat/1.lp", 49),
    ("simple", "examples/ilp/instances/mps/sat/1.mps", 49),
    ("simple", "examples/ilp/instances/lp/unsat/1.lp", False),
    ("simple", "examples/ilp/instances/mps/unsat/1.mps", False),
]

for kind, instance, expected in instances:
    no_lprelax = "--no-lprelax" if False else ""
    expected = f"--expected-objective {expected}" if expected is not False else "--expected-unsat"

    cmd = shlex.split(solver_cmd.format(instance=instance, no_lprelax=no_lprelax, expected=expected))
    print("Solving instance: " + instance)
    solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, universal_newlines=True)
    if solver_run.returncode != 0:
        print("Solver did not return expected result")
        exit(1)
