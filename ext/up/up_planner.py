import sys
import subprocess
import time
import grpc
from typing import IO, Callable, Optional

# Use the local version of the UP in the `ext/up/unified_planning` git submodule
from upf.model import ProblemKind

sys.path.insert(0, 'unified_planning')


import unified_planning as up
import unified_planning.engines as engines
import unified_planning.engines.mixins as mixins
from unified_planning.engines.results import LogLevel, PlanGenerationResult, PlanGenerationResultStatus
import unified_planning.grpc.generated.unified_planning_pb2 as proto
import unified_planning.grpc.generated.unified_planning_pb2_grpc as grpc_api
from unified_planning.grpc.proto_writer import ProtobufWriter
from unified_planning.grpc.proto_reader import ProtobufReader

from unified_planning.shortcuts import *
from unified_planning.model.htn import *


class GRPCPlanner(engines.engine.Engine, mixins.OneshotPlannerMixin):
    """
    This class is the interface of a generic gRPC planner
    that can be contacted at a given host and port.
    """

    def __init__(self, host: str = "localhost", port: Optional[int] = None):
        engines.engine.Engine.__init__(self)
        mixins.OneshotPlannerMixin.__init__(self)
        self._host = host
        self._port = port
        self._writer = ProtobufWriter()
        self._reader = ProtobufReader()

    def _solve(self, problem: 'up.model.AbstractProblem',
               callback: Optional[Callable[['up.engines.results.PlanGenerationResult'], None]] = None,
               timeout: Optional[float] = None,
               output_stream: Optional[IO[str]] = None) -> 'up.engines.results.PlanGenerationResult':
        assert isinstance(problem, up.model.Problem)
        proto_problem = self._writer.convert(problem)
        with grpc.insecure_channel(f'{self._host}:{self._port}') as channel:
            planner = grpc_api.UnifiedPlanningStub(channel)
            req = proto.PlanRequest(problem=proto_problem, timeout=timeout)
            response_stream = planner.planOneShot(req)
            for response in response_stream:
                response = self._reader.convert(response, problem)
                assert isinstance(response, up.engines.results.PlanGenerationResult)
                if response.status == PlanGenerationResultStatus.INTERMEDIATE and callback is not None:
                    callback(response)
                else:
                    return response


aries_path = '/home/abitmonnot/work/aries'
aries_build_cmd = f"cargo build --profile ci --bin up-server"
aries_exe = f'target/ci/up-server'
log_file = "/tmp/log-aries"


class AriesLocal(GRPCPlanner):
    def __init__(self, port: int):
        print("Compiling...")
        build = subprocess.Popen(aries_build_cmd, shell=True, cwd=aries_path)
        build.wait()
        logs = open(log_file, "w")
        print(f"Launching Aries gRPC server (logs at {log_file})...")
        # subprocess.Popen([f"{aries_exe}"], cwd=aries_path, shell=True, stdout=logs, stderr=logs)
        subprocess.Popen([f"{aries_exe}"], cwd=aries_path, shell=True, stdout=sys.stdout, stderr=sys.stderr)
        time.sleep(.1)
        GRPCPlanner.__init__(self, host="localhost", port=port)

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        supported_kind = up.model.ProblemKind()
        supported_kind.set_problem_class('ACTION_BASED')
        supported_kind.set_problem_class('HIERARCHICAL')
        supported_kind.set_time('CONTINUOUS_TIME')
        supported_kind.set_time('INTERMEDIATE_CONDITIONS_AND_EFFECTS')
        supported_kind.set_time('TIMED_EFFECT')
        supported_kind.set_time('TIMED_GOALS')
        supported_kind.set_time('DURATION_INEQUALITIES')
        #supported_kind.set_numbers('DISCRETE_NUMBERS')
        #supported_kind.set_numbers('CONTINUOUS_NUMBERS')
        supported_kind.set_typing('FLAT_TYPING')
        supported_kind.set_typing('HIERARCHICAL_TYPING')
        supported_kind.set_conditions_kind('NEGATIVE_CONDITIONS')
        supported_kind.set_conditions_kind('DISJUNCTIVE_CONDITIONS')
        supported_kind.set_conditions_kind('EQUALITY')
        #supported_kind.set_fluents_type('NUMERIC_FLUENTS')
        supported_kind.set_fluents_type('OBJECT_FLUENTS')
        supported_kind.set_quality_metrics('ACTIONS_COST')
        supported_kind.set_quality_metrics('MAKESPAN')
        supported_kind.set_quality_metrics('PLAN_LENGTH')
        return supported_kind

    @staticmethod
    def supports(problem_kind: 'up.model.ProblemKind') -> bool:
        return problem_kind <= AriesLocal.supported_kind()


def problem():
    x = Fluent('x')
    y = Fluent('y')
    action_costs = {}
    a = InstantaneousAction('a')
    a.add_precondition(Not(x))
    a.add_effect(x, True)
    action_costs[a] = Int(10)

    b = InstantaneousAction('b')
    b.add_precondition(Not(y))
    b.add_effect(y, True)
    action_costs[b] = Int(1)

    c = InstantaneousAction('c')
    c.add_precondition(y)
    c.add_effect(x, True)
    action_costs[c] = Int(1)

    problem = Problem('basic_with_costs')
    problem.add_fluent(x)
    problem.add_fluent(y)
    problem.add_action(a)
    problem.add_action(b)
    problem.add_action(c)
    problem.set_initial_value(x, False)
    problem.set_initial_value(y, False)
    problem.add_goal(x)
    problem.add_quality_metric(up.model.metrics.MinimizeActionCosts(action_costs))
    return problem


def cost(problem, plan):
    if len(problem.quality_metrics) == 0:
        return None
    assert len(problem.quality_metrics) == 1
    metric = problem.quality_metrics[0]
    if isinstance(metric, up.model.metrics.MinimizeActionCosts):
        return sum([metric.get_action_cost(action_instance.action).int_constant_value() for _, action_instance, _ in plan.timed_actions])
    else:
        raise ValueError("Unsupported metric: ", metric)


if __name__ == "__main__":
    from unified_planning.test.examples import get_example_problems

    instances = [
        "basic",
        "basic_without_negative_preconditions",
        "basic_nested_conjunctions",
        "hierarchical_blocks_world",
        "hierarchical_blocks_world_object_as_root",
        "hierarchical_blocks_world_with_object",
        "matchcellar",
        "htn-go"
    ]

    planner = AriesLocal(port=2222)

    def plan(problem):
        print(f"\n==== {problem.name} ====")
        result = planner.solve(problem)

        print("Answer: ", result.status)
        if result.plan:
            for start, action, duration in result.plan.timed_actions:
                if duration:
                    print("%s: %s [%s]" % (float(start), action, float(duration)))
                else:
                    print("%s: %s" % (float(start), action))
            print("\nCost: ", cost(problem, result.plan))

    # for instance in instances:
    #     problem = get_example_problems()[instance].problem
    #     plan(problem)

    plan(problem())

