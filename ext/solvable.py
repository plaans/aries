#!/usr/bin/python3

# Script that should be run from the root of the repository.
# It tries to solve all files named "instance-1.pddl" in the ext directory.
# If it solved in less than 10 seconds and the returned plan is valid,
# the corresponding domain and problem are added to the 'problems/pddl/ipc'
# directory.

from pathlib import Path
import os
import subprocess
import tempfile
import re
import shutil
from termcolor import colored
import sys


def hddl_candidates():
    domain_dirs = []
    for base_dir in [Path("ext/hddl/total-order"), Path("ext/hddl/partial-order")]:
        domain_dirs += [f.path for f in os.scandir(base_dir) if f.is_dir()]

    # add the first problem of each domain directory
    hddl_problems = []
    for dom in domain_dirs:
        # print(dom)
        pbs = [f.path for f in os.scandir(dom)
               if f.is_file()
               and f.name.endswith('.hddl')
               and f.name.find('domain') == -1]
        pbs.sort()
        hddl_problems.append(pbs[0])

    hddl_problems.sort()
    return hddl_problems


def pddl_ipc_year_name(problem_file):
    re_year_name = "ext/pddl/ipc-(?P<year>.*)/domains/(?P<name>.*)/instances/instance-1.pddl"
    match = re.search(re_year_name, str(problem_file))
    return match.group("year"), match.group("name")


def hddl_ipc_year_name(problem_file):
    re_year_name = "ext/hddl/(?P<order>.*)-order/(?P<name>.*)/.*"
    match = re.search(re_year_name, str(problem_file))
    if match.group("order") == "total":
        prefix = "to-"
    else:
        prefix = "po-"
    return "2020", (prefix + str(match.group("name")))


def domain_of(path):
    result = subprocess.run(['./target/release/planning-domain', path], stdout=subprocess.PIPE, text=True)
    if result.returncode == 0:
        return result.stdout
    else:
        return None


os.system("cargo build --release --bin lcp")
os.system("cargo build --release --bin planning-domain")

MODE = sys.argv[1]

if MODE == "LCP":
    pattern = 'instance-1.pddl'
    solver_cmd = "timeout 10s ./target/release/lcp -d {domain} {problem} -o {plan}"
    validation_cmd = "./ext/val-pddl -v {domain} {problem} {plan}"
    year_name = pddl_ipc_year_name
    outdir = Path("problems/pddl/ipc")
    candidates = [path for path in Path('ext').rglob(pattern)]
    candidates.sort()
    extension = ".pddl"
elif MODE == "HDDL":
    solver_cmd = "timeout 20s ./target/release/lcp -d {domain} {problem} -o {plan}"
    validation_cmd = "./ext/val-hddl -l -verify {domain} {problem} {plan}"
    year_name = hddl_ipc_year_name
    outdir = Path("problems/hddl/ipc")
    candidates = hddl_candidates()
    extension = ".hddl"
else:
    print("UNKNOWN MODE: " + str(MODE))
    exit(1)

print("# Problems that will be attempted")
for pb in candidates:
    print("  " + str(pb))

solved = []

print("\n# Solving\n")

for pb in candidates:
    (year, name) = year_name(pb)
    header = "\n\n======> " + str(year) + " / " + str(name) + "\n"
    print(colored(header, 'red'))
    domain = domain_of(pb)
    if not domain:
        print("Domain not found")
        continue
    plan_file = tempfile.NamedTemporaryFile().name
    attributes = {
        'domain': domain,
        'problem': pb,
        'plan': plan_file
    }
    cmd = solver_cmd.format(**attributes).split(" ")
    solver_run = subprocess.run(cmd, stdout=subprocess.PIPE, text=True)
    if solver_run.returncode != 0:
        print("NOT SOLVED")
        continue
    print("Solved")
    cmd = validation_cmd.format(**attributes).split(" ")
    val_run = subprocess.run(cmd, stdout=subprocess.PIPE, text=True)
    if val_run.returncode != 0:
        print("INVALID PLAN RETURNED")
        print("====== SOLVER LOG =======")
        print(solver_run.stdout)
        print("====== VAL LOG =======")
        print(val_run.stdout)
        # exit(1)
    else:
        target = outdir / (year + "-" + name)
        if not target.exists():
            target.mkdir(parents=True)
            shutil.copyfile(domain, target / ("domain" + extension))
            shutil.copyfile(pb, target / ("instance-1" + extension))
        solved.append(pb)


print("======= ALL SOLVED AND VALIDATED")
for s in solved:
    print(s)







