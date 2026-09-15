from ortools.linear_solver.python import model_builder
import os
import subprocess
import json
import time
from tabulate import tabulate


def get_cargo_target_dir() -> str:
    cmd = ["cargo", "metadata", "--format-version", "1", "--no-deps"]
    res = subprocess.run(cmd, capture_output=True, text=True, check=True)
    metadata = json.loads(res.stdout)
    return metadata["target_directory"]

def solve_cmd(cmd, timeout):
    value = None
    try:
        start = time.time()
        aries_lp_return = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        end = time.time()
        
        exec_time_aries = end - start

        if aries_lp_return.returncode != 0:
            print(f"Error while parsing {filename}")
            return ("PARSE-ERROR", None, None)


        words = aries_lp_return.stdout.split(" ")
        status = words[1]

        if status == 'OPTIMAL':
            value = float(words[3])
    except subprocess.TimeoutExpired:
        return ("TIMEOUT", None, None)

    return (status, value, exec_time_aries)

def compare_ref(status_ref, value_ref, status, value):
    agree = True
    if status == status_ref:
        if status == 'OPTIMAL':
            if abs((value - value_ref) / value_ref) < TOLERANCE:
                status = COLOR_AGREE + status + ENDC
            else:
                agree = False
        else :
            status = COLOR_AGREE + status + ENDC
    else:
        agree = False

    return (status, agree)

COLOR_AGREE = '\033[32m'
ENDC = '\033[m'
TIMEOUT = 120
MPS_DIR = 'mps_files'

PROFILE = 'perf'

build_cmd = f"cargo build --example solve_mps --profile {PROFILE}".split(" ")

if subprocess.run(build_cmd).returncode:
    print("Error while building aries-lp")
    exit(1)
    
target_dir = get_cargo_target_dir()

solver_cmd = f"{target_dir}/{PROFILE}/examples/solve_mps " + "{mps_dir}/{instance}"

TOLERANCE = 0.005 # Percentage of the objective value

table_print = [['Instance', 'ORTools', 'value', 'exec time', 'aries-lp', 'value', 'exec time', 'aries-lp-incr', 'value', 'exec time']]
table_file = [['Instance', 'ORTools', 'value', 'exec time', 'aries-lp', 'value', 'exec time', 'aries-lp-incr', 'value', 'exec time']]

for filename in os.listdir('mps_files'):

    if not filename.endswith('.mps'):
        continue

    print(f"Solving: {filename}")


    value_or = None

    model = model_builder.ModelBuilder()

    # 1. Import the MPS file
    if not model.import_from_mps_file(f"{MPS_DIR}/{filename}"):
        print(f"Error while parsing {filename}")
    else:
        # 2. Init the solver
        solver = model_builder.ModelSolver('glop')
        
        # 3. Solve
        start = time.time()
        status_or = solver.solve(model)
        end = time.time()

        exec_time_or = end - start

        if status_or == model_builder.SolveStatus.OPTIMAL:
            value_or = solver.objective_value

        
    cmd = solver_cmd.format(mps_dir=MPS_DIR, instance=filename).split(" ")
    cmd_incr = cmd + ["--incremental"]

    (status_aries, value_aries, exec_time_aries) = solve_cmd(cmd, TIMEOUT)
    (status_aries_incr, value_aries_incr, exec_time_aries_incr) = solve_cmd(cmd_incr, TIMEOUT)

    # We do not want color characters in the file
    table_file.append([filename, status_or.name, value_or, exec_time_or, status_aries, value_aries, exec_time_aries, status_aries_incr, value_aries_incr, exec_time_aries_incr])

    all_agree = True

    (status_aries, agree) = compare_ref(status_or.name, value_or, status_aries, value_aries)

    all_agree = all_agree and agree

    (status_aries_incr, agree) = compare_ref(status_or.name, value_or, status_aries_incr, value_aries_incr)

    all_agree = all_agree and agree

    if all_agree:
        filename = COLOR_AGREE + filename + ENDC

    table_print.append([filename, status_or.name, value_or, exec_time_or, status_aries, value_aries, exec_time_aries, status_aries_incr, value_aries_incr, exec_time_aries_incr])

            


with open("results.txt", "w") as f:
    f.write(tabulate(table_file , headers='firstrow', tablefmt='fancy_grid'))

print(tabulate(table_print, headers='firstrow', tablefmt='fancy_grid'))