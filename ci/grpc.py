#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the GRPC server with a set of binaries from the UPF platform

import os
import subprocess

os.system("cargo build --profile ci --bin upf-server")
solver = "target/ci/upf-server"

solver_cmd = solver + " {instance}"

instances = ["./ext/grpc/bins/robot_problem.bin", "./ext/grpc/bins/robot.bin"]
# instances = []
# for instance in os.listdir("./ext/grpc/bins"):
#     if instance.endswith(".bin"):
#         instances.append(os.path.join("./ext/grpc/bins", instance))

for instance in instances:
    cmd = solver_cmd.format(instance=instance).split(" ")
    print("Solving instance: " + instance)
    print("Command: " + " ".join(cmd))
    solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, universal_newlines=True)
    if solver_run.returncode != 0:
        print("Solver did not return expected result")
        exit(1)