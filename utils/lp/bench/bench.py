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

COLOR_AGREE = '\033[32m'
ENDC = '\033[m'

mps_dir = 'mps_files'

profile = 'perf'

build_cmd = f"cargo build --example solve_mps --profile {profile}".split(" ")

if subprocess.run(build_cmd).returncode:
    print("Error while building aries-lp")
    exit(1)
    
target_dir = get_cargo_target_dir()

solver_cmd = f"{target_dir}/{profile}/examples/solve_mps " + "{mps_dir}/{instance}"

TOLERANCE = 0.001 # Percentage of the objective value

nb_solved = 0
nb_agree_value = 0
nb_files = 0

table_print = [['Instance', 'ORTools', 'value', 'exec time', 'aries-lp', 'value', 'exec time']]
table_file = [['Instance', 'ORTools', 'value', 'exec time', 'aries-lp', 'value', 'exec time']]

for filename in os.listdir('mps_files'):

    if not filename.endswith('.mps'):
        continue

    print(f"Solving: {filename}")

    nb_files += 1

    value_or = None

    model = model_builder.ModelBuilder()

    # 1. Import the MPS file
    if not model.import_from_mps_file(f"{mps_dir}/{filename}"):
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
        
    value_aries = None

    cmd = solver_cmd.format(mps_dir=mps_dir, instance=filename).split(" ")

    start = time.time()
    aries_lp_return = subprocess.run(cmd, capture_output=True, text=True)
    end = time.time()
    
    exec_time_aries = end - start

    if aries_lp_return.returncode != 0:
            print(f"Error while parsing {filename}")
            continue


    words = aries_lp_return.stdout.split(" ")
    status_aries = words[1]

    if status_aries == 'OPTIMAL':
        value_aries = float(words[3])

    # We do not want color characters in the file
    table_file.append([filename, status_or.name, value_or, exec_time_or, status_aries, value_aries, exec_time_aries])

    if status_aries == status_or.name:
        if status_aries == 'OPTIMAL':
            nb_solved += 1
            if abs((value_aries - value_or) / value_or) < TOLERANCE:
                nb_agree_value += 1
                filename = COLOR_AGREE + filename + ENDC
                status_aries = COLOR_AGREE + status_aries + ENDC
        else :
            filename = COLOR_AGREE + filename + ENDC
            status_aries = COLOR_AGREE + status_aries + ENDC

    table_print.append([filename, status_or.name, value_or, exec_time_or, status_aries, value_aries, exec_time_aries])

            


with open("results.txt", "w") as f:
    f.write(tabulate(table_file , headers='firstrow', tablefmt='fancy_grid'))

print(tabulate(table_print, headers='firstrow', tablefmt='fancy_grid'))

print(f"Number files: {nb_files}")
print(f"Number both solved: {nb_solved}, agree on value: {nb_agree_value}")