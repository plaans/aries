#!/usr/bin/env python3

# pylint: disable=missing-function-docstring, missing-module-docstring, missing-class-docstring
# pylint: disable=too-few-public-methods, redefined-outer-name

from dataclasses import dataclass
import os
from pathlib import Path
from typing import Generator, Optional

import pytest
from unified_planning.engines.engine import Engine
from unified_planning.engines.results import (
    PlanGenerationResult,
    PlanGenerationResultStatus,
)
from unified_planning.io.pddl_reader import PDDLReader
from unified_planning.plans.plan import Plan, PlanKind
from unified_planning.shortcuts import AnytimePlanner, OneshotPlanner, Problem


@dataclass(frozen=True)
class WarmUpScenario:
    uid: str
    problem: Problem
    plan: str
    quality: float
    timeout: int = 5

    def __str__(self):
        return self.uid

    def __repr__(self):
        return f"WarmUpScenario({self.uid})"

    def __iter__(self):
        return iter((self.problem, self.plan))


@dataclass(frozen=True)
class PlanningResult:
    idx: int
    status: PlanGenerationResultStatus
    plan: Optional[Plan]
    quality: Optional[float]
    elapsed_time: float

    def __post_init__(self):
        print(self)

    def __str__(self):
        quality = str(self.quality) if self.quality is not None else "N/A"
        time = f"{self.elapsed_time:.2f}" if self.elapsed_time >= 0 else "N/A"
        return f"{self.idx: <8}{self.status.name: <24}{quality: <16}{time: <16}"

    def __repr__(self):
        data = self.__dict__.copy()
        data["status"] = self.status.name
        del data["plan"]
        txt = ", ".join(f"{k}={v}" for k, v in data.items())
        return f"PlanningResult({txt})"

    @classmethod
    def from_upf(cls, problem: Problem, result: PlanGenerationResult, idx: int = 0):
        return cls(
            idx=idx,
            status=result.status,
            plan=result.plan,
            quality=cls.compute_quality(problem, result.plan),
            elapsed_time=float(result.metrics.get("engine_internal_time", -1)),
        )

    @staticmethod
    def compute_quality(problem: Problem, plan: Optional[Plan]) -> Optional[float]:
        # NOTE: Assume the quality is the makespan.
        if plan is None:
            return None

        if plan.kind == PlanKind.SEQUENTIAL_PLAN:
            return len(plan.actions)

        if plan.kind == PlanKind.TIME_TRIGGERED_PLAN:
            if (
                "CONTINUOUS_TIME" in problem.kind.features
                or "DISCRETE_TIME" in problem.kind.features
            ):
                return float(max(s + (d or 0) for (s, _, d) in plan.timed_actions))
            return len(plan.timed_actions)

        raise ValueError(f"Unsupported plan kind: {plan.kind}")


def _scenarios() -> Generator[WarmUpScenario, None, None]:
    fixtures_dir = Path(__file__).parent / "fixtures/warm_up"
    for domain_dir in fixtures_dir.iterdir():
        if not domain_dir.is_dir():
            continue
        domain_file = domain_dir / "domain.pddl"
        problem_file = domain_dir / "problem.pddl"
        for plan_file in domain_dir.glob("plan_*.txt"):
            problem = PDDLReader().parse_problem(domain_file, problem_file)
            plan = plan_file.read_text()
            quality = float(plan_file.stem.split("_")[-1])
            uid = f"{domain_dir.name}/{quality}"
            yield WarmUpScenario(uid=uid, problem=problem, plan=plan, quality=quality)


@pytest.fixture(params=_scenarios(), ids=lambda s: s.uid)
def scenario(request):
    yield request.param


def oneshot_planning(problem: Problem, plan: str, timeout: int) -> PlanningResult:
    with OneshotPlanner(name="aries", params={"warm_up_plan": plan}) as planner:
        planner.skip_checks = True
        solution = planner.solve(problem, timeout=timeout)
    return PlanningResult.from_upf(problem, solution)


def anytime_planning(
    problem: Problem, plan: str, timeout: int
) -> Generator[PlanningResult, None, None]:
    with AnytimePlanner(name="aries", params={"warm_up_plan": plan}) as planner:
        planner.skip_checks = True
        for idx, solution in enumerate(planner.get_solutions(problem, timeout=timeout)):
            yield PlanningResult.from_upf(problem, solution, idx)


class TestAriesWarmUp:
    def setup(self):
        os.environ["ARIES_UP_ASSUME_REALS_ARE_INTS"] = "true"
        os.environ["ARIES_LCP_SYMMETRY_BREAKING"] = "simple"
        print("\n        STATUS                  QUALITY         TIME")

    @pytest.fixture(autouse=True, scope="function")
    def fixture_method(self):
        self.setup()
        yield


class TestAriesStrictWarmUp(TestAriesWarmUp):
    def setup(self):
        super().setup()
        os.environ["ARIES_WARM_UP"] = "strict"

    def test_oneshot_returns_same_plan(self, scenario: WarmUpScenario):
        problem, plan = scenario
        result = oneshot_planning(problem, plan, scenario.timeout)
        assert str(result.plan) == str(plan)
        assert result.quality == scenario.quality

    def test_anytime_first_plan_is_same(self, scenario: WarmUpScenario):
        problem, plan = scenario
        first_result = next(anytime_planning(problem, plan, scenario.timeout))
        assert str(first_result.plan) == str(plan)
        assert first_result.quality == scenario.quality

    def test_anytime_improves_plan_over_time(self, scenario: WarmUpScenario):
        problem, plan = scenario
        best = scenario.quality + 0.1
        for result in anytime_planning(problem, plan, scenario.timeout):
            if result.status != PlanGenerationResultStatus.INTERMEDIATE:
                continue
            assert result.quality is not None
            assert result.quality < best
            best = result.quality
        assert best is not None
