#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the solvers finds the appropriate result for instances
# in the examples/sat/problems/cnf/{sat.zip, unsat.zip} archives.

import os
import subprocess
import time

os.system("cargo build --profile ci --bin aries-sat")
solver = "target/ci/aries-sat"

solver_cmd = solver + " {params} --source {archive} {instance}"


def files_in_archive(archive):
    res = subprocess.run(["zipinfo", "-1", str(archive)], stdout=subprocess.PIPE, universal_newlines=True)
    if res.returncode != 0:
        exit(1)
    return res.stdout.split()


def run_all(archive, sat):
    for instance in files_in_archive(archive):
        if sat:
            print("Solving   SAT:    " + str(instance), end='', flush=True)
            params = "--sat true"
        else:
            print("Solving UNSAT:    " + str(instance), end='', flush=True)
            params = "--sat false"
        start = time.time()
        cmd = solver_cmd.format(params=params, archive=archive, instance=instance).split(" ")
        solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, universal_newlines=True)
        end = time.time()
        duration = int((end - start) * 1000)
        print(f"\t[{duration} ms]")
        if solver_run.returncode != 0:
            print("Solver did not return expected result")
            exit(1)


run_all("examples/sat/instances/test-sat.zip", sat=True)
run_all("examples/sat/instances/test-unsat.zip", sat=False)

