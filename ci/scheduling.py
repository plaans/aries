#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It validates that the jobshop solver find the optimal solution for a few instances.

import os
import subprocess

os.system("cargo build --profile ci --bin scheduler")
solver = "target/ci/scheduler"

solver_cmd = solver + " {kind} {instance} --expected-makespan {makespan}"

instances = [
    ("jobshop", "examples/scheduling/instances/jobshop/ft06.jsp", 55),
    ("jobshop", "examples/scheduling/instances/jobshop/la01.jsp", 666),
    ("jobshop", "examples/scheduling/instances/jobshop/la02.jsp", 655),
    ("jobshop", "examples/scheduling/instances/jobshop/la03.jsp", 597),
    ("jobshop", "examples/scheduling/instances/jobshop/la04.jsp", 590),
    ("jobshop", "examples/scheduling/instances/jobshop/la05.jsp", 593),
    ("jobshop", "examples/scheduling/instances/jobshop/la06.jsp", 926),
    # ("jobshop", "examples/scheduling/instances/jobshop/la07.jsp", 890), # too costly to solve
    ("jobshop", "examples/scheduling/instances/jobshop/la08.jsp", 863),
    ("jobshop", "examples/scheduling/instances/jobshop/la09.jsp", 951),
    ("jobshop", "examples/scheduling/instances/jobshop/la10.jsp", 958),
    ("jobshop", "examples/scheduling/instances/jobshop/la11.jsp", 1222),
    ("jobshop", "examples/scheduling/instances/jobshop/la12.jsp", 1039),
    ("jobshop", "examples/scheduling/instances/jobshop/la13.jsp", 1150),
    ("jobshop", "examples/scheduling/instances/jobshop/orb05.jsp", 887),
    ("jobshop", "examples/scheduling/instances/jobshop/ta01.jsp", 1231),
    ("jobshop", "examples/scheduling/instances/jobshop/ta02.jsp", 1244),

    ("openshop", "examples/scheduling/instances/openshop/taillard/tai04_04_01.osp", 193),

    ("flexible", "examples/scheduling/instances/flexible/hu/edata/mt06.fjs", 55),
    ("flexible", "examples/scheduling/instances/flexible/hu/edata/la01.fjs", 609),
    ("flexible", "examples/scheduling/instances/flexible/hu/edata/la02.fjs", 655),
    ("flexible", "examples/scheduling/instances/flexible/hu/rdata/mt06.fjs", 47),
    ("flexible", "examples/scheduling/instances/flexible/hu/rdata/la16.fjs", 717),
    ("flexible", "examples/scheduling/instances/flexible/hu/rdata/la17.fjs", 646),
    ("flexible", "examples/scheduling/instances/flexible/hu/vdata/mt06.fjs", 47),

]

for (kind, instance, makespan) in instances:
    cmd = solver_cmd.format(kind=kind, instance=instance, makespan=makespan).split(" ")
    print("Solving instance: " + instance)
    solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, universal_newlines=True)
    if solver_run.returncode != 0:
        print("Solver did not return expected result")
        exit(1)
