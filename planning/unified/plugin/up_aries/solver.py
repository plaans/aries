#!/usr/bin/env python3
"""Unified Planning Integration for Aries"""
import os
import platform
import signal
import subprocess
import time
from typing import IO, Dict, Optional, Set, Tuple, Type, Union

from unified_planning.engines.mixins.oneshot_planner import OptimalityGuarantee

import socket
import threading
from typing import IO, Callable, Dict, Optional, Set, Tuple, Type

import unified_planning as up
import unified_planning.engines.mixins as mixins
import unified_planning.grpc.generated.unified_planning_pb2 as proto
import unified_planning.grpc.generated.unified_planning_pb2_grpc as grpc_api
from unified_planning import engines
from unified_planning.engines.results import PlanGenerationResultStatus
from unified_planning.exceptions import UPException
from unified_planning.grpc.proto_reader import ProtobufReader  # type: ignore[attr-defined]
from unified_planning.grpc.proto_writer import ProtobufWriter  # type: ignore[attr-defined]

import grpc


class GRPCPlanner(engines.engine.Engine, mixins.OneshotPlannerMixin):
    """Represents the GRPC interface that must be implemented by the planner"""

    _instances: Dict[Tuple[Optional[int], Type["GRPCPlanner"]], "GRPCPlanner"]
    _ports: Set[int]
    _lock = threading.Lock()

    def __init__(
        self,
        host: str = "localhost",
        port: Optional[int] = None,
        override: bool = False,
        timeout: Optional[float] = 0.5,
    ) -> None:
        """GRPC Planner Definition
        :param host: Host address, defaults to "localhost"
        :type host: str, optional
        :param port: Port, defaults to None
        :type port: Optional[int], optional
        :param override: Override the creation of new client, defaults to False
        :type override: bool, optional
        :param timeout: Timeout in seconds, defaults to 0.5
        :type timeout: Optional[float], optional
        :raises UPException: If the gRPC server is not available or accessible
        """
        self._host = host
        self._port = port
        self._override = override
        self._timeout_sec = timeout

        self._writer = ProtobufWriter()
        self._reader = ProtobufReader()

        # Setup channel
        self._channel = grpc.insecure_channel(
            f"{self._host}:{self._port}",
            options=(
                ("grpc.enable_http_proxy", 0),
                ("grpc.so_reuseport", 0),
            ),
        )
        if not self._grpc_server_on(self._channel):
            raise UPException(
                "The GRPC server is not available on port {}".format(self._port)
            )

        self._planner = grpc_api.UnifiedPlanningStub(self._channel)

    def __new__(cls, **kwargs) -> "GRPCPlanner":
        """Create a thread-safe singleton instance of the planner
        Modes:
         - If parameters are provided,
                - If the port is available, create a new client
                - If the port is already in use, return the existing client
         - If no parameters are provided, use default.
        """
        port = kwargs.get("port", None)

        if (port, cls) not in cls._instances:
            with cls._lock:
                if cls not in cls._instances:
                    cls._ports.add(port)
                    cls._instances[(port, cls)] = super().__new__(cls)
                    return cls._instances[(port, cls)]
        elif kwargs.get("override", False):
            return super().__new__(cls)
        return cls._instances[(port, cls)]

    def __del__(self) -> None:
        """Delete the planner instance"""
        self._channel.close()

        for instance in self._instances:
            if instance[1] == self:
                self._instances.pop(instance)
                break

    def _solve(
        self,
        problem: "up.model.AbstractProblem",
        heuristic: Optional[
            Callable[["up.model.state.ROState"], Optional[float]]
        ] = None,
        timeout: Optional[float] = None,
        output_stream: Optional[IO[str]] = None,
    ) -> "up.engines.results.PlanGenerationResult":
        """GRPC Client for Unified Planning
        :param problem: Problem to solve
        :type problem: up.model.AbstractProblem
        :param heuristic:  is a function that given a state returns its heuristic value or `None` if the state is a dead-end, defaults to `None`.
        :type heuristic: Optional[Callable[["up.model.state.ROState"], Optional[float]], None], Optional
        :param timeout: Timeout in seconds, defaults to None
        :type timeout: Optional[float], optional
        :param output_stream: Log output stream, defaults to None
        :type output_stream: Optional[IO[str]], optional
        :return: Plan generation result
        :rtype: up.engines.results.PlanGenerationResult
        """

        # Assert that the problem is a valid problem
        assert isinstance(problem, up.model.Problem)

        proto_problem = self._writer.convert(problem)

        req = proto.PlanRequest(problem=proto_problem, timeout=timeout)
        response_stream = self._planner.planOneShot(req)
        response = self._reader.convert(response_stream, problem)
        assert isinstance(response, up.engines.results.PlanGenerationResult)
        if (
            response.status == PlanGenerationResultStatus.INTERMEDIATE
            and heuristic is not None
        ):
            # TODO: Implement heuristic
            pass
        else:
            return response

        raise UPException("No response from the server")

    def _grpc_server_on(self, channel) -> bool:
        """Check if the grpc server is available
        :param channel: UP GRPC Channel
        :type channel: grpc.Channel
        :return: True if the server is available, False otherwise
        :rtype: bool
        """
        try:
            grpc.channel_ready_future(channel).result(timeout=self._timeout_sec)
            return True
        except grpc.FutureTimeoutError:
            return False

    @classmethod
    def get_available_port(cls) -> int:
        """Get an available port for the GRPC server
        :return: Available port
        :rtype: int
        """
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.bind(("", 0))
            return s.getsockname()[1]


_EXECUTABLES = {
    ("Linux", "x86_64"): "bin/up-aries_linux_amd64",
    ("Linux", "aarch64"): "bin/up-aries_linux_arm64",
    ("Darwin", "x86_64"): "bin/up-aries_macos_amd64",
    ("Darwin", "aarch64"): "bin/up-aries_macos_arm64",
    ("Darwin", "arm64"): "bin/up-aries_macos_arm64",
    ("Windows", "AMD64"): "bin/up-aries_windows_amd64.exe",
    ("Windows", "aarch64"): "bin/up-aries_windows_arm64.exe",
}


def _executable():
    """Locates the Aries executable to use for the current platform."""
    try:
        filename = _EXECUTABLES[(platform.system(), platform.machine())]
    except KeyError:
        raise OSError(f"No executable for this platform: {platform.system()} / {platform.machine()}")
    exe = os.path.join(os.path.dirname(__file__), filename)
    if not os.path.exists(exe) or not os.path.isfile(exe):
        raise FileNotFoundError(f"Could not locate executable: {exe}")
    return exe


class Aries(GRPCPlanner):
    """Represents the solver interface."""

    reader = ProtobufReader()
    writer = ProtobufWriter()
    _instances: Dict[Tuple[Optional[int], Type["GRPCPlanner"]], "GRPCPlanner"] = {}
    _ports: Set[int] = set()

    def __init__(
        self,
        host: str = "localhost",
        port: int = 2222,
        override: bool = True,
        stdout: Optional[IO[str]] = None,
    ):
        """Initialize the Aries solver."""
        if stdout is None:
            self.stdout = open(os.devnull, "w")

        host = "127.0.0.1" if host == "localhost" else host
        self.optimality_metric_required = False
        self.executable = _executable()

        self.process_id = subprocess.Popen(
            f"{self.executable} --address {host}:{port}",
            stdout=self.stdout,
            stderr=self.stdout,
            shell=True,
        )
        time.sleep(0.3)  # 0.1s fails on macOS
        super().__init__(host=host, port=port, override=override)

    @property
    def name(self) -> str:
        return "aries"

    @staticmethod
    def is_oneshot_planner() -> bool:
        return True

    @staticmethod
    def satisfies(optimality_guarantee: Union[OptimalityGuarantee, str]) -> bool:
        # TODO: Optimality Integrity
        return super().satisfies(optimality_guarantee)

    @staticmethod
    def is_plan_validator() -> bool:
        return False

    @staticmethod
    def is_grounder() -> bool:
        return False

    def ground(self, problem: "up.model.Problem") -> "up.solvers.results.GroundingResult":
        raise UPException("Aries does not support grounding")

    def validate(
        self, problem: "up.model.Problem", plan: "up.plan.Plan"
    ) -> "up.solvers.results.ValidationRes":
        raise UPException("Aries does not support validation")

    @staticmethod
    def supports(problem_kind: "up.model.ProblemKind") -> bool:
        supported_kind = up.model.ProblemKind()
        supported_kind.set_problem_class("ACTION_BASED")  # type: ignore
        supported_kind.set_problem_class("HIERARCHICAL")  # type: ignore
        supported_kind.set_time("CONTINUOUS_TIME")  # type: ignore
        supported_kind.set_time("INTERMEDIATE_CONDITIONS_AND_EFFECTS")  # type: ignore
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

        return problem_kind <= supported_kind

    def _skip_checks(self) -> bool:
        return False

    def destroy(self):
        """Destroy the solver."""
        if self.process_id is not None:
            self.process_id.send_signal(signal.SIGTERM)
            self.process_id = None

        if self.stdout is not None:
            self.stdout.close()
            self.stdout = None

        # Free port if still in use
        subprocess.run(["fuser", "-k", "-n", "tcp", str(self._port)])

    def __del__(self):
        super().__del__()
        self.destroy()
