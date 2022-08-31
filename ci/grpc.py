#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the GRPC server with a set of binaries from the UPF platform

import os
import subprocess

build_result = os.system("cargo build --profile ci --bin up-server")
if build_result != 0:
    exit(1)

solver = "target/ci/up-server"

solver_cmd = solver + " 0.0.0.0:2222 {instance}"

instances = [
    "basic",
    "basic_without_negative_preconditions",
    "basic_nested_conjunctions",
    "hierarchical_blocks_world",
    "hierarchical_blocks_world_object_as_root",
    "hierarchical_blocks_world_with_object",
    "matchcellar"
]
problem_files = [f"./ext/up/bins/problems/{name}.bin" for name in instances]

for problem_file in problem_files:
    cmd = solver_cmd.format(instance=problem_file).split(" ")
    print("Solving instance: " + problem_file)
    print("Command: " + " ".join(cmd))
    solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, universal_newlines=True)
    if solver_run.returncode != 0:
        print("Solver did not return expected result")
        exit(1)
