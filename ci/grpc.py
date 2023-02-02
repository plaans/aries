#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the GRPC server with a set of binaries from the UPF platform

import os
import subprocess
import argparse
from pathlib import Path


parser = argparse.ArgumentParser(description="Run GRPC server.")
parser.add_argument(
    "--executable", help="Path to the executable to run", default=None, nargs="?"
)

args = parser.parse_args()
executable = args.executable if args.executable else "target/ci/up-server"

if not args.executable:
    build_result = os.system("cargo build --profile ci --bin up-server")
    if build_result != 0:
        exit(1)

    solver = "target/ci/up-server"
else:
    solver = os.path.abspath(args.executable)

solver_cmd = solver + " --address 0.0.0.0:2222 --file-path {instance}"

problem_dir = Path("./planning/ext/up/bins/problems/").resolve()
problem_files = list(map(str, list(problem_dir.iterdir())))

errors: dict[str, str] = {}
for problem_file in problem_files:
    cmd = solver_cmd.format(instance=problem_file).split(" ")
    print("Solving instance: " + problem_file)
    print("Command: " + " ".join(cmd))
    solver_run = subprocess.run(
        cmd,
        stderr=subprocess.PIPE,
        stdout=subprocess.PIPE,
        universal_newlines=True,
    )
    if solver_run.returncode != 0:
        problem_name = Path(problem_file).name
        errors[problem_name] = solver_run.stderr
        print(errors[problem_name])
if len(errors) != 0:
    print(f"\n===== {len(errors)} errors on {len(problem_files)} problems =====")
    print("\n".join(f"{k}\n{v}" for k, v in errors.items()))
exit(len(errors) == 0)
