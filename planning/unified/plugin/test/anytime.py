# Example script of the usage of anytime planning

from unified_planning.model import Problem
from unified_planning.model.metrics import MinimizeSequentialPlanLength
from up_aries import Aries
from unified_planning.io import PDDLReader

dir = "../../../problems/hddl/ipc/2020-po-Rover"
reader = PDDLReader()
pb: Problem = reader.parse_problem(f"{dir}/domain.hddl", f"{dir}/instance.1.pb.hddl")

pb.add_quality_metric(MinimizeSequentialPlanLength())
print(pb)


planner = Aries()
for sol in planner.get_solutions(pb):
    print()
    print(sol.status, "length: ", len(sol.plan.actions))
    print(sol.plan)