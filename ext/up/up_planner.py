#!/usr/bin/python3

import subprocess
import sys
import time
from pathlib import Path
from typing import IO, Callable, Optional

import grpc
from test_problems import problems

# Use the local version of the UP in the `ext/up/unified_planning` git submodule
UP_DIR = Path(__file__).parent.resolve() / "unified_planning"
if UP_DIR not in sys.path:
    sys.path.insert(0, UP_DIR.as_posix())


import unified_planning as up
import unified_planning.engines as engines
import unified_planning.engines.mixins as mixins
import unified_planning.grpc.generated.unified_planning_pb2 as proto
import unified_planning.grpc.generated.unified_planning_pb2_grpc as grpc_api
from unified_planning.engines.results import PlanGenerationResultStatus
from unified_planning.grpc.proto_reader import ProtobufReader
from unified_planning.grpc.proto_writer import ProtobufWriter
from unified_planning.model.htn import *
from unified_planning.shortcuts import *


# TODO: move to upstream
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

    def _solve(
        self,
        problem: "up.model.AbstractProblem",
        callback: Optional[
            Callable[["up.engines.results.PlanGenerationResult"], None]
        ] = None,
        timeout: Optional[float] = None,
        output_stream: Optional[IO[str]] = None,
    ) -> "up.engines.results.PlanGenerationResult":
        assert isinstance(problem, up.model.Problem)
        proto_problem = self._writer.convert(problem)
        with grpc.insecure_channel(f"{self._host}:{self._port}") as channel:
            planner = grpc_api.UnifiedPlanningStub(channel)
            req = proto.PlanRequest(problem=proto_problem, timeout=timeout)
            response_stream = planner.planOneShot(req)
            for response in response_stream:
                response = self._reader.convert(response, problem)
                assert isinstance(response, up.engines.results.PlanGenerationResult)
                if response.status != PlanGenerationResultStatus.INTERMEDIATE:
                    return response
                elif callback:
                    callback(response)
                else:
                    pass  # Intermediate plan but no callback


aries_path = Path(__file__).parent.parent.parent.as_posix()
aries_build_cmd = "cargo build --profile ci --bin up-server"
aries_exe = (Path(aries_path) / "target/ci/up-server").as_posix()
log_file = "/tmp/log-aries"


class AriesLocal(GRPCPlanner):
    """This class implements a specific gRPC solver that will compile and launch Aries from sources in the current directory."""

    def __init__(
        self,
        port: int,
        stdout: Optional[IO[str]] = None,
        compilation: bool = True,
    ):
        # if compilation:
        if compilation:
            print("Compiling...")
            build = subprocess.Popen(aries_build_cmd, shell=True, cwd=aries_path)
            build.wait()

        logs = stdout or open(log_file, mode="w", encoding="utf-8")
        print(f"Launching Aries gRPC server (logs at {log_file})...")
        subprocess.Popen(
            [aries_exe, "-p", str(port)],
            cwd=aries_path,
            # shell=True,
            stdout=logs,
            stderr=logs,
        )
        # subprocess.Popen([f"{aries_exe}"], cwd=aries_path, shell=True, stdout=sys.stdout, stderr=sys.stderr)
        time.sleep(0.1)  # Wait to make sure the server is up and running
        GRPCPlanner.__init__(self, host="localhost", port=port)

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        supported_kind = up.model.ProblemKind()
        supported_kind.set_problem_class("ACTION_BASED")
        supported_kind.set_problem_class("HIERARCHICAL")
        supported_kind.set_time("CONTINUOUS_TIME")
        supported_kind.set_time("INTERMEDIATE_CONDITIONS_AND_EFFECTS")
        supported_kind.set_time("TIMED_EFFECT")
        supported_kind.set_time("TIMED_GOALS")
        supported_kind.set_time("DURATION_INEQUALITIES")
        # supported_kind.set_numbers('DISCRETE_NUMBERS')
        # supported_kind.set_numbers('CONTINUOUS_NUMBERS')
        supported_kind.set_typing("FLAT_TYPING")
        supported_kind.set_typing("HIERARCHICAL_TYPING")
        supported_kind.set_conditions_kind("NEGATIVE_CONDITIONS")
        supported_kind.set_conditions_kind("DISJUNCTIVE_CONDITIONS")
        supported_kind.set_conditions_kind("EQUALITY")
        # supported_kind.set_fluents_type('NUMERIC_FLUENTS')
        supported_kind.set_fluents_type("OBJECT_FLUENTS")
        supported_kind.set_quality_metrics("ACTIONS_COST")
        supported_kind.set_quality_metrics("MAKESPAN")
        supported_kind.set_quality_metrics("PLAN_LENGTH")
        return supported_kind

    @staticmethod
    def supports(problem_kind: "up.model.ProblemKind") -> bool:
        return problem_kind <= AriesLocal.supported_kind()


# TODO: move to upstream
def cost(problem, plan):
    """Computes the cost of a plan"""
    if len(problem.quality_metrics) == 0:
        return None
    assert len(problem.quality_metrics) == 1
    metric = problem.quality_metrics[0]
    if metric == None:
        return None
    if isinstance(metric, up.model.metrics.MinimizeActionCosts):
        return sum(
            [
                metric.get_action_cost(action_instance.action).int_constant_value()
                for _, action_instance, _ in plan.timed_actions
            ]
        )
    elif isinstance(metric, up.model.metrics.MinimizeMakespan):
        if isinstance(plan, up.plans.TimeTriggeredPlan):
            return max(
                [start + (dur if dur else 0) for start, _, dur in plan.timed_actions]
                + [0]
            )
        else:
            raise ValueError(
                "The makespan metric is only defined for time-triggerered plan"
            )

    else:
        raise ValueError("Unsupported metric: ", metric)


if __name__ == "__main__":
    planner = AriesLocal(port=2222)

    def plan(problem, expected_cost=None):
        print(problem)
        print(f"\n==== {problem.name} ====")
        result = planner.solve(
            problem,
            callback=lambda p: print(
                "New plan with cost: ", cost(problem, p), flush=True
            ),
        )

        print("Answer: ", result.status)
        if result.plan:
            for start, action, duration in result.plan.timed_actions:
                if duration:
                    print("%s: %s [%s]" % (float(start), action, float(duration)))
                else:
                    print("%s: %s" % (float(start), action))
            c = cost(problem, result.plan)
            expected = (
                f"(expected: {expected_cost})" if expected_cost is not None else ""
            )
            print("\nCost: ", c, expected)
            assert expected_cost is None or c == expected_cost

    # Run on some test problems of AIPlan4EU
    from unified_planning.test.examples import get_example_problems

    instances = [
        "basic",
        "basic_without_negative_preconditions",
        "basic_nested_conjunctions",
        "hierarchical_blocks_world",
        "hierarchical_blocks_world_object_as_root",
        "hierarchical_blocks_world_with_object",
        "matchcellar",
        "htn-go",
    ]
    for instance in instances:
        problem = get_example_problems()[instance].problem
        plan(problem)

    # Run on some of our own problem with an expected solution cost
    for problem, c in problems():
        plan(problem, expected_cost=c)
