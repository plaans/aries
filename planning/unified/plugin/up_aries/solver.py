#!/usr/bin/env python3
"""Unified Planning Integration for Aries"""
import os
import platform
import socket
import subprocess
import tempfile
import time
from fractions import Fraction
from pathlib import Path
from typing import IO, Callable, Optional, Iterator

import grpc
import unified_planning as up
import unified_planning.engines.mixins as mixins
import unified_planning.grpc.generated.unified_planning_pb2 as proto
import unified_planning.grpc.generated.unified_planning_pb2_grpc as grpc_api
from unified_planning import engines
from unified_planning.engines import PlanGenerationResultStatus, AnytimeGuarantee
from unified_planning.engines.mixins.oneshot_planner import OptimalityGuarantee
from unified_planning.grpc.proto_reader import (
    ProtobufReader,
)  # type: ignore[attr-defined]
from unified_planning.grpc.proto_writer import (
    ProtobufWriter,
)  # type: ignore[attr-defined]
from unified_planning.plans import PlanKind

_EXECUTABLES = {
    ("Linux", "x86_64"): "bin/up-aries_linux_amd64",
    ("Linux", "aarch64"): "bin/up-aries_linux_arm64",
    ("Darwin", "x86_64"): "bin/up-aries_macos_amd64",
    ("Darwin", "aarch64"): "bin/up-aries_macos_arm64",
    ("Darwin", "arm64"): "bin/up-aries_macos_arm64",
    ("Windows", "AMD64"): "bin/up-aries_windows_amd64.exe",
    ("Windows", "aarch64"): "bin/up-aries_windows_arm64.exe",
}
_DEV_ENV_VAR = "UP_ARIES_DEV"

# Boolean flag that is set to true on the first compilation of the Aries server.
_ARIES_PREVIOUSLY_COMPILED = False

_ARIES_EPSILON = Fraction(1, 10)

_ARIES_SUPPORTED_KIND = up.model.ProblemKind({
    # PROBLEM_CLASS
    "ACTION_BASED",
    "HIERARCHICAL",
    # "CONTINGENT", "ACTION_BASED_MULTI_AGENT", "SCHEDULING", "TAMP",
    # PROBLEM_TYPE
    # "SIMPLE_NUMERIC_PLANNING", "GENERAL_NUMERIC_PLANNING",
    # TIME
    "CONTINUOUS_TIME",
    "DISCRETE_TIME",
    "INTERMEDIATE_CONDITIONS_AND_EFFECTS",
    "EXTERNAL_CONDITIONS_AND_EFFECTS",
    "TIMED_EFFECTS", "TIMED_EFFECT",  # backward compat
    "TIMED_GOALS",
    "DURATION_INEQUALITIES",
    # EXPRESSION_DURATION
    # "STATIC_FLUENTS_IN_DURATIONS", "STATIC_FLUENTS_IN_DURATION", # backward compat
    # "FLUENTS_IN_DURATIONS", "FLUENTS_IN_DURATION",  # backward compat
    # NUMBERS
    # "CONTINUOUS_NUMBERS",
    # "DISCRETE_NUMBERS",
    # "BOUNDED_TYPES",
    # CONDITIONS_KIND
    "NEGATIVE_CONDITIONS",
    "DISJUNCTIVE_CONDITIONS",
    "EQUALITIES", "EQUALITY",  # backward compat
    # "EXISTENTIAL_CONDITIONS",
    # "UNIVERSAL_CONDITIONS",
    # EFFECTS_KIND
    # "CONDITIONAL_EFFECTS",
    # "INCREASE_EFFECTS",
    # "DECREASE_EFFECTS",
    "STATIC_FLUENTS_IN_BOOLEAN_ASSIGNMENTS",
    "STATIC_FLUENTS_IN_NUMERIC_ASSIGNMENTS",
    "FLUENTS_IN_BOOLEAN_ASSIGNMENTS",
    "FLUENTS_IN_NUMERIC_ASSIGNMENTS",
    # TYPING
    "FLAT_TYPING",
    "HIERARCHICAL_TYPING",
    # FLUENTS_TYPE
    # "NUMERIC_FLUENTS",
    "OBJECT_FLUENTS",
    # QUALITY_METRICS
    "ACTIONS_COST",
    # "FINAL_VALUE",
    "MAKESPAN",
    "PLAN_LENGTH",
    # "OVERSUBSCRIPTION",
    # "TEMPORAL_OVERSUBSCRIPTION",
    # ACTIONS_COST_KIND
    # "STATIC_FLUENTS_IN_ACTIONS_COST",
    # "FLUENTS_IN_ACTIONS_COST",
    # SIMULATED_ENTITIES
    # "SIMULATED_EFFECTS",
    # CONSTRAINTS_KIND
    # "TRAJECTORY_CONSTRAINTS",
    # HIERARCHICAL
    "METHOD_PRECONDITIONS",
    "TASK_NETWORK_CONSTRAINTS",
    "INITIAL_TASK_NETWORK_VARIABLES",
    "TASK_ORDER_TOTAL",
    "TASK_ORDER_PARTIAL",
    # "TASK_ORDER_TEMPORAL",
})

_ARIES_VAL_SUPPORTED_KIND = up.model.ProblemKind({
    # PROBLEM_CLASS
    "ACTION_BASED",
    "HIERARCHICAL",
    # PROBLEM_TYPE
    "SIMPLE_NUMERIC_PLANNING",
    "GENERAL_NUMERIC_PLANNING",
    # TIME
    "CONTINUOUS_TIME",
    "DISCRETE_TIME",
    "INTERMEDIATE_CONDITIONS_AND_EFFECTS",
    "EXTERNAL_CONDITIONS_AND_EFFECTS",
    "TIMED_EFFECTS", "TIMED_EFFECT",  # backward compat
    "TIMED_GOALS",
    "DURATION_INEQUALITIES",
    # EXPRESSION_DURATION
    "STATIC_FLUENTS_IN_DURATIONS", "STATIC_FLUENTS_IN_DURATION",  # backward compat
    "FLUENTS_IN_DURATIONS", "FLUENTS_IN_DURATION",  # backward compat
    # NUMBERS
    "CONTINUOUS_NUMBERS",
    "DISCRETE_NUMBERS",
    # "BOUNDED_TYPES",
    # CONDITIONS_KIND
    "NEGATIVE_CONDITIONS",
    "DISJUNCTIVE_CONDITIONS",
    "EQUALITIES", "EQUALITY",  # backward compat
    "EXISTENTIAL_CONDITIONS",
    "UNIVERSAL_CONDITIONS",
    # EFFECTS_KIND
    "CONDITIONAL_EFFECTS",
    "INCREASE_EFFECTS",
    "DECREASE_EFFECTS",
    "STATIC_FLUENTS_IN_BOOLEAN_ASSIGNMENTS",
    "STATIC_FLUENTS_IN_NUMERIC_ASSIGNMENTS",
    "FLUENTS_IN_BOOLEAN_ASSIGNMENTS",
    "FLUENTS_IN_NUMERIC_ASSIGNMENTS",
    # TYPING
    "FLAT_TYPING",
    "HIERARCHICAL_TYPING",
    # FLUENTS_TYPE
    "NUMERIC_FLUENTS",
    "OBJECT_FLUENTS",
    # QUALITY_METRICS
    "ACTIONS_COST",
    "FINAL_VALUE",
    "MAKESPAN",
    "PLAN_LENGTH",
    "OVERSUBSCRIPTION",
    "TEMPORAL_OVERSUBSCRIPTION",
    # ACTIONS_COST_KIND
    "STATIC_FLUENTS_IN_ACTIONS_COST",
    "FLUENTS_IN_ACTIONS_COST",
    # SIMULATED_ENTITIES
    # "SIMULATED_EFFECTS",
    # CONSTRAINTS_KIND
    # "TRAJECTORY_CONSTRAINTS",
    # HIERARCHICAL
    "METHOD_PRECONDITIONS",
    # "TASK_NETWORK_CONSTRAINTS",
    # "INITIAL_TASK_NETWORK_VARIABLES",
    "TASK_ORDER_TOTAL",
    "TASK_ORDER_PARTIAL",
    "TASK_ORDER_TEMPORAL",
})


def _is_dev() -> bool:
    env_str = os.getenv(_DEV_ENV_VAR, "false").lower()
    if env_str in ("true", "t", "1"):
        return True
    if env_str in ("false", "f", "0"):
        return False
    raise ValueError(f"Unknown value {env_str} for {_DEV_ENV_VAR}, expected a boolean")


def _find_executable() -> str:
    """Locates the Aries executable to use for the current platform."""
    try:
        filename = _EXECUTABLES[(platform.system(), platform.machine())]
    except KeyError as err:
        raise OSError(
            f"No executable for this platform: {platform.system()} / {platform.machine()}"
        ) from err
    exe = os.path.join(os.path.dirname(__file__), filename)
    if not os.path.exists(exe) or not os.path.isfile(exe):
        raise FileNotFoundError(f"Could not locate executable: {exe}")
    return exe


class AriesEngine(engines.engine.Engine):
    """Base class for all Aries engines."""

    _reader = ProtobufReader()
    _writer = ProtobufWriter()
    _host = "127.0.0.1"

    def __init__(self, executable: Optional[str] = None, **kwargs):
        """Initialize the Aries solver."""
        if _is_dev():
            executable = self._compile()
            kwargs.setdefault("host", "localhost")
            kwargs.setdefault("port", 2222)
            kwargs.setdefault("override", True)
        super().__init__(**kwargs)
        self.optimality_metric_required = False
        self._executable = executable if executable is not None else _find_executable()

    def _compile(self) -> str:
        global _ARIES_PREVIOUSLY_COMPILED
        # Search the root of the aries project.
        # resolve() makes the path absolute, resolving all symlinks on the way.
        aries_path = Path(__file__).resolve().parent.parent.parent.parent.parent
        aries_exe = aries_path / "target/ci/up-server"

        if not _ARIES_PREVIOUSLY_COMPILED:
            aries_build_cmd = "cargo build --profile ci --bin up-server"
            print(f"Compiling Aries ({aries_path}) ...")
            with open(os.devnull, "w", encoding="utf-8") as stdout:
                subprocess.run(
                    aries_build_cmd,
                    shell=True,
                    cwd=aries_path,
                    stdout=stdout,
                )
            _ARIES_PREVIOUSLY_COMPILED = True
        return aries_exe.as_posix()


class Aries(AriesEngine, mixins.OneshotPlannerMixin, mixins.AnytimePlannerMixin):
    """Represents the solver interface."""

    @property
    def name(self) -> str:
        return "aries"

    def _solve(
        self,
        problem: "up.model.AbstractProblem",
        heuristic: Optional[
            Callable[["up.model.state.ROState"], Optional[float]]
        ] = None,
        timeout: Optional[float] = None,
        output_stream: Optional[IO[str]] = None,
    ) -> "up.engines.results.PlanGenerationResult":
        # Assert that the problem is a valid problem
        assert isinstance(problem, up.model.Problem)
        if heuristic is not None:
            print(
                "Warning: The aries solver does not support custom heuristic (as it is not a state-space planner)."
            )

        # start a gRPC server in its own process
        # Note: when the `server` object is garbage collected, the process will be killed
        server = _Server(self._executable, output_stream=output_stream)
        proto_problem = self._writer.convert(problem)

        req = proto.PlanRequest(problem=proto_problem, timeout=timeout)
        response = server.planner.planOneShot(req)
        response = self._reader.convert(response, problem)

        # if we have a time triggered plan and a recent version of the UP that support setting epsilon-separation,
        # send the result through an additional (in)validation to ensure it meets the minimal separation
        if isinstance(response.plan, up.plans.TimeTriggeredPlan) \
                and "correct_plan_generation_result" in dir(up.engines.results):
            response = up.engines.results.correct_plan_generation_result(
                response,
                problem,
                _ARIES_EPSILON,
            )

        return response

    def _get_solutions(self, problem: "up.model.AbstractProblem", timeout: Optional[float] = None,
                       output_stream: Optional[IO[str]] = None) -> Iterator["up.engines.results.PlanGenerationResult"]:
        # Assert that the problem is a valid problem
        assert isinstance(problem, up.model.Problem)

        # start a gRPC server in its own process
        # Note: when the `server` object is garbage collected, the process will be killed
        server = _Server(self._executable, output_stream=output_stream)
        proto_problem = self._writer.convert(problem)

        req = proto.PlanRequest(problem=proto_problem, timeout=timeout)
        stream = server.planner.planAnytime(req)
        for response in stream:
            response = self._reader.convert(response, problem)
            yield response
            # The parallel solver implementation in aries are such that intermediate answer might arrive late
            if response.status != PlanGenerationResultStatus.INTERMEDIATE:
                break  # definitive answer, exit

    @staticmethod
    def satisfies(optimality_guarantee: OptimalityGuarantee) -> bool:
        # in general, we cannot provide optimality guarantees except for non-recursive HTNs
        return optimality_guarantee == OptimalityGuarantee.SATISFICING

    @staticmethod
    def ensures(anytime_guarantee: AnytimeGuarantee) -> bool:
        return anytime_guarantee == AnytimeGuarantee.INCREASING_QUALITY

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        return _ARIES_SUPPORTED_KIND

    @staticmethod
    def supports(problem_kind: up.model.ProblemKind) -> bool:
        return problem_kind <= Aries.supported_kind()


class AriesVal(AriesEngine, mixins.PlanValidatorMixin):
    """Represents the validator interface."""

    @property
    def name(self) -> str:
        return "aries-val"

    def _validate(
        self, problem: "up.model.AbstractProblem", plan: "up.plans.Plan"
    ) -> "up.engines.results.ValidationResult":
        # start a gRPC server in its own process
        # Note: when the `server` object is garbage collected, the process will be killed
        server = _Server(self._executable)
        proto_problem = self._writer.convert(problem)
        proto_plan = self._writer.convert(plan)

        req = proto.ValidationRequest(problem=proto_problem, plan=proto_plan)
        response = server.planner.validatePlan(req)
        response = self._reader.convert(response)
        return response

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        return _ARIES_VAL_SUPPORTED_KIND

    @staticmethod
    def supports(problem_kind: up.model.ProblemKind) -> bool:
        return problem_kind <= AriesVal.supported_kind()

    @staticmethod
    def supports_plan(plan_kind: PlanKind) -> bool:
        supported_plans = [
            PlanKind.SEQUENTIAL_PLAN,
            PlanKind.TIME_TRIGGERED_PLAN,
            # PlanKind.PARTIAL_ORDER_PLAN,
            # PlanKind.CONTINGENT_PLAN,
            # PlanKind.STN_PLAN,
            PlanKind.HIERARCHICAL_PLAN,
        ]
        return plan_kind in supported_plans


def _get_available_port() -> int:
    """Get an available port for the GRPC server
    :return: Available port
    :rtype: int
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("", 0))
        return s.getsockname()[1]


class _Server:
    """This class is used to manage the lifetime of a planning server.
    When instantiated, a new process will be started, exposing the gRPC interface on an arbitrary port.
    Once we are connected to this server, the initialization method will return and the resulting object will
    have a functional gRPC interface in its `planner` attribute.

    When the `_Server` object is garbage collected, the planner's process is killed
    """

    def __init__(self, executable: str, output_stream: Optional[IO[str]] = None):
        # start = time.time()
        host = "127.0.0.1"
        port = _get_available_port()
        if output_stream is None:
            # log to a file '/tmp/aries-{PORT}.XXXXXXXXX'
            output_stream = tempfile.NamedTemporaryFile(
                mode="w", prefix=f"aries-{port}.", delete=False
            )
        cmd = f"{executable} --address {host}:{port}"
        self._process = subprocess.Popen(
            cmd.split(" "),
            stdout=output_stream,
            stderr=output_stream,
        )

        channel = grpc.insecure_channel(f"{host}:{port}")
        try:
            # wait for connection to be available (at most 2 second)
            # let 10ms elapse before trying, to maximize the chances that the server be up on hte first try
            # this is a workaround since, if the server is the server is not up on the first try, the
            # `channel_ready_future` method apparently waits 1 second before retrying
            time.sleep(0.01)
            grpc.channel_ready_future(channel).result(2)
        except grpc.FutureTimeoutError as err:
            raise up.exceptions.UPException(
                "Error: failed to connect to Aries solver through gRPC."
            ) from err
        # establish connection
        self.planner = grpc_api.UnifiedPlanningStub(channel)
        # end = time.time()
        # print("Initialization time: ", end-start, "seconds")

    def __del__(self):
        # On garbage collection, kill the planner's process
        self._process.kill()
