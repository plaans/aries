#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the jobshop solver find the optimal solution for a few instances.

import os
import subprocess

os.system("cargo build --bin aries-jobshop")
solver = "target/debug/aries-jobshop"

solver_cmd = solver + " examples/jobshop/instances/{instance}.txt --expected-makespan {makespan}"

instances = [("ft06", 55), ("la01", 666)]

for (instance, makespan) in instances:
    cmd = solver_cmd.format(instance=instance, makespan=makespan).split(" ")
    print("Solving instance: " + instance)
    solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, universal_newlines=True)
    if solver_run.returncode != 0:
        print("Solver did not return expected result")
        exit(1)


