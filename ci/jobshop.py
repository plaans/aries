#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the jobshop solver find the optimal solution for a few instances.

import os
import subprocess

os.system("cargo build --profile ci --bin scheduler")
solver = "target/ci/scheduler"

solver_cmd = solver + " {kind} {instance} --expected-makespan {makespan}"

instances = [
    ("jobshop", "examples/scheduling/instances/jobshop/ft06.txt", 55),
    ("jobshop", "examples/scheduling/instances/jobshop/la01.txt", 666),
    ("openshop", "examples/scheduling/instances/openshop/taillard/tai04_04_01.txt", 193),
]

for (kind, instance, makespan) in instances:
    cmd = solver_cmd.format(kind=kind, instance=instance, makespan=makespan).split(" ")
    print("Solving instance: " + instance)
    solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, universal_newlines=True)
    if solver_run.returncode != 0:
        print("Solver did not return expected result")
        exit(1)


