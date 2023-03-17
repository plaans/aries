#!/usr/bin/env python3
"""Unified Planning Integration for Aries"""
import os
import platform
import socket
import subprocess
import tempfile
import time
from pathlib import Path
from typing import IO, Callable, Optional

import grpc
import unified_planning as up
import unified_planning.engines.mixins as mixins
import unified_planning.grpc.generated.unified_planning_pb2 as proto
import unified_planning.grpc.generated.unified_planning_pb2_grpc as grpc_api
from unified_planning import engines
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
                build = subprocess.Popen(
                    aries_build_cmd,
                    shell=True,
                    cwd=aries_path,
                    stdout=stdout,
                )
                build.wait()
            _ARIES_PREVIOUSLY_COMPILED = True
        return aries_exe.as_posix()


class Aries(AriesEngine, mixins.OneshotPlannerMixin):
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
        return response

    @staticmethod
    def satisfies(optimality_guarantee: OptimalityGuarantee) -> bool:
        # in general, we cannot provide optimality guarantees except for non-recursive HTNs
        return optimality_guarantee == OptimalityGuarantee.SATISFICING

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        supported_kind = up.model.ProblemKind()
        supported_kind.set_problem_class("ACTION_BASED")  # type: ignore
        supported_kind.set_problem_class("HIERARCHICAL")  # type: ignore
        supported_kind.set_time("CONTINUOUS_TIME")  # type: ignore
        supported_kind.set_time("INTERMEDIATE_CONDITIONS_AND_EFFECTS")  # type: ignore
        supported_kind.set_time("EXTERNAL_CONDITIONS_AND_EFFECTS")  # type: ignore
        supported_kind.set_time("TIMED_EFFECT")  # type: ignore
        supported_kind.set_time("TIMED_GOALS")  # type: ignore
        supported_kind.set_time("DURATION_INEQUALITIES")  # type: ignore
        # supported_kind.set_numbers('DISCRETE_NUMBERS') # type: ignore
        # supported_kind.set_numbers('CONTINUOUS_NUMBERS') # type: ignore
        supported_kind.set_typing("FLAT_TYPING")  # type: ignore
        supported_kind.set_typing("HIERARCHICAL_TYPING")  # type: ignore
        supported_kind.set_conditions_kind("NEGATIVE_CONDITIONS")  # type: ignore
        supported_kind.set_conditions_kind("DISJUNCTIVE_CONDITIONS")  # type: ignore
        supported_kind.set_conditions_kind("EQUALITY")  # type: ignore
        # supported_kind.set_fluents_type('NUMERIC_FLUENTS') # type: ignore
        supported_kind.set_fluents_type("OBJECT_FLUENTS")  # type: ignore
        supported_kind.set_hierarchical("METHOD_PRECONDITIONS")
        supported_kind.set_hierarchical("TASK_NETWORK_CONSTRAINTS")
        supported_kind.set_hierarchical("INITIAL_TASK_NETWORK_VARIABLES")
        supported_kind.set_hierarchical("TASK_ORDER_TOTAL")
        supported_kind.set_hierarchical("TASK_ORDER_PARTIAL")
        # supported_kind.set_hierarchical("TASK_ORDER_TEMPORAL")
        supported_kind.set_quality_metrics("ACTIONS_COST")
        supported_kind.set_quality_metrics("MAKESPAN")
        supported_kind.set_quality_metrics("PLAN_LENGTH")
        return supported_kind

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
        supported_kind = up.model.ProblemKind()
        # Problem class
        supported_kind.set_problem_class("ACTION_BASED")  # type: ignore
        supported_kind.set_problem_class("HIERARCHICAL")  # type: ignore
        # Problem type
        supported_kind.set_problem_type("SIMPLE_NUMERIC_PLANNING")  # type: ignore
        supported_kind.set_problem_type("GENERAL_NUMERIC_PLANNING")  # type: ignore
        # Time
        supported_kind.set_time("CONTINUOUS_TIME")  # type: ignore
        supported_kind.set_time("DISCRETE_TIME")  # type: ignore
        supported_kind.set_time("INTERMEDIATE_CONDITIONS_AND_EFFECTS")  # type: ignore
        supported_kind.set_time("EXTERNAL_CONDITIONS_AND_EFFECTS")  # type: ignore
        supported_kind.set_time("TIMED_EFFECT")  # type: ignore
        supported_kind.set_time("TIMED_GOALS")  # type: ignore
        supported_kind.set_time("DURATION_INEQUALITIES")  # type: ignore
        # Expression duration
        supported_kind.set_expression_duration("STATIC_FLUENTS_IN_DURATION")  # type: ignore
        supported_kind.set_expression_duration("FLUENTS_IN_DURATION")  # type: ignore
        # Numbers
        supported_kind.set_numbers("CONTINUOUS_NUMBERS")  # type: ignore
        supported_kind.set_numbers("DISCRETE_NUMBERS")  # type: ignore
        # Conditions kind
        supported_kind.set_conditions_kind("NEGATIVE_CONDITIONS")  # type: ignore
        supported_kind.set_conditions_kind("DISJUNCTIVE_CONDITIONS")  # type: ignore
        supported_kind.set_conditions_kind("EQUALITY")  # type: ignore
        supported_kind.set_conditions_kind("EXISTENTIAL_CONDITIONS")  # type: ignore
        supported_kind.set_conditions_kind("UNIVERSAL_CONDITIONS")  # type: ignore
        # Effects kind
        supported_kind.set_effects_kind("CONDITIONAL_EFFECTS")
        supported_kind.set_effects_kind("INCREASE_EFFECTS")
        supported_kind.set_effects_kind("DECREASE_EFFECTS")
        # Typing
        supported_kind.set_typing("FLAT_TYPING")  # type: ignore
        supported_kind.set_typing("HIERARCHICAL_TYPING")  # type: ignore
        # Fluents type
        supported_kind.set_fluents_type("NUMERIC_FLUENTS")  # type: ignore
        supported_kind.set_fluents_type("OBJECT_FLUENTS")  # type: ignore
        # Quality metrics
        supported_kind.set_quality_metrics("ACTIONS_COST")
        supported_kind.set_quality_metrics("FINAL_VALUE")
        supported_kind.set_quality_metrics("MAKESPAN")
        supported_kind.set_quality_metrics("PLAN_LENGTH")
        supported_kind.set_quality_metrics("OVERSUBSCRIPTION")
        # Hierarchical
        supported_kind.set_hierarchical("METHOD_PRECONDITIONS")
        # supported_kind.set_hierarchical("TASK_NETWORK_CONSTRAINTS")
        # supported_kind.set_hierarchical("INITIAL_TASK_NETWORK_VARIABLES")
        supported_kind.set_hierarchical("TASK_ORDER_TOTAL")
        supported_kind.set_hierarchical("TASK_ORDER_PARTIAL")
        supported_kind.set_hierarchical("TASK_ORDER_TEMPORAL")
        return supported_kind

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
