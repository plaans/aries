import sys

# Use the local version of the UP in the `ext/up/unified_planning` git submodule
from upf.model import ProblemKind

sys.path.insert(0, 'unified_planning')

import grpc
import unified_planning as up
import unified_planning.engines as engines
import unified_planning.engines.mixins as mixins
from unified_planning.engines.results import LogLevel, PlanGenerationResult, PlanGenerationResultStatus
from typing import IO, Callable, Optional


import unified_planning.grpc.generated.unified_planning_pb2 as proto
import unified_planning.grpc.generated.unified_planning_pb2_grpc as grpc_api
from unified_planning.grpc.proto_writer import ProtobufWriter
from unified_planning.grpc.proto_reader import ProtobufReader


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



class Aries(GRPCPlanner):
    def __init__(self, port: int):
        GRPCPlanner.__init__(self, host="localhost", port=port)

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        supported_kind = up.model.ProblemKind()
        supported_kind.set_problem_class('ACTION_BASED') # type: ignore
        supported_kind.set_problem_class('HIERARCHICAL') # type: ignore
        supported_kind.set_time('CONTINUOUS_TIME') # type: ignore
        supported_kind.set_time('INTERMEDIATE_CONDITIONS_AND_EFFECTS') # type: ignore
        supported_kind.set_time('TIMED_EFFECT') # type: ignore
        supported_kind.set_time('TIMED_GOALS') # type: ignore
        supported_kind.set_time('DURATION_INEQUALITIES') # type: ignore
        #supported_kind.set_numbers('DISCRETE_NUMBERS') # type: ignore
        #supported_kind.set_numbers('CONTINUOUS_NUMBERS') # type: ignore
        supported_kind.set_typing('FLAT_TYPING') # type: ignore
        supported_kind.set_typing('HIERARCHICAL_TYPING') # type: ignore
        supported_kind.set_conditions_kind('NEGATIVE_CONDITIONS') # type: ignore
        supported_kind.set_conditions_kind('DISJUNCTIVE_CONDITIONS') # type: ignore
        supported_kind.set_conditions_kind('EQUALITY') # type: ignore
        #supported_kind.set_fluents_type('NUMERIC_FLUENTS') # type: ignore
        supported_kind.set_fluents_type('OBJECT_FLUENTS') # type: ignore
        return supported_kind

    @staticmethod
    def supports(problem_kind: 'up.model.ProblemKind') -> bool:
        return problem_kind <= Aries.supported_kind()


if __name__ == "__main__":
    from unified_planning.test.examples import get_example_problems
    print("starting...")

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

    planner = Aries(port=2222)

    def plan(instance):
        problem = get_example_problems()[instance].problem
        print(f"\n==== {instance} ====")
        result = planner.solve(problem)

        print("Answer: ", result.status)
        if result.plan:
            for start, action, duration in result.plan.timed_actions:
                if duration:
                    print("%s: %s [%s]" % (float(start), action, float(duration)))
                else:
                    print("%s: %s" % (float(start), action))

    for instance in instances:
        plan(instance)
