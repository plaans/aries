#!/usr/bin/env python3
"""Unified Planning Integration for Aries"""
import os
import signal
import subprocess
import time
from typing import IO, Dict, Optional, Set, Tuple, Type, Union

from up_aries.executor import Executor

import unified_planning as up
from unified_planning.engines.mixins.oneshot_planner import OptimalityGuarantee
from unified_planning.exceptions import UPException
from unified_planning.grpc.proto_reader import ProtobufReader
from unified_planning.grpc.proto_writer import ProtobufWriter
from unified_planning.grpc.server import GRPCPlanner


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
        self.executable = os.path.join(os.path.dirname(__file__), Executor()())

        self.process_id = subprocess.Popen(
            f"{self.executable} --address {host}:{port}",
            stdout=self.stdout,
            stderr=self.stdout,
            shell=True,
        )
        time.sleep(0.1)
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

    def ground(self, problem: "up.model.Problem") -> "up.solvers.results.GroundingResu":
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
            self.process_id.send_signal(signal.SIGINT)
            self.process_id = None

        if self.stdout is not None:
            self.stdout.close()
            self.stdout = None

        # Free port if still in use
        subprocess.run(["fuser", "-k", "-n", "tcp", str(self._port)])

    def __del__(self):
        super().__del__()
        self.destroy()
