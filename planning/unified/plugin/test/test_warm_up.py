#!/usr/bin/env python3

# pylint: disable=missing-function-docstring, missing-module-docstring, missing-class-docstring
# pylint: disable=too-few-public-methods, redefined-outer-name

import contextlib
from dataclasses import dataclass
import os
from pathlib import Path
from typing import Generator, Optional

import pytest
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
    timeout: int = 300

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
            problem.name = domain_dir.name
            plan = plan_file.read_text()
            quality = float(plan_file.stem.split("_")[-1])
            uid = f"{domain_dir.name}/{quality}"
            yield WarmUpScenario(uid=uid, problem=problem, plan=plan, quality=quality)


@pytest.fixture(params=_scenarios(), ids=lambda s: s.uid)
def scenario(request):
    yield request.param


def get_output_file(scenario: WarmUpScenario, planning_mode: str) -> Path:
    uid = scenario.uid.replace("/", "-")
    warm_up_mode = os.environ.get("ARIES_WARM_UP", "unknown")
    return Path(f"/tmp/aries-{warm_up_mode}-{planning_mode}-{uid}.log")


def oneshot_planning(
    scenario: WarmUpScenario, use_warm_up: bool = True
) -> PlanningResult:
    output_file = get_output_file(scenario, "oneshot")
    params = {"warm_up_plan": scenario.plan} if use_warm_up else {}
    with (
        open(output_file, "w", encoding="utf-8") as output_stream,
        OneshotPlanner(name="aries", params=params) as planner,
    ):
        planner.skip_checks = True
        solution = planner.solve(
            scenario.problem,
            timeout=scenario.timeout,
            output_stream=output_stream,
        )
    return PlanningResult.from_upf(scenario.problem, solution)


def anytime_planning(
    scenario: WarmUpScenario, use_warm_up: bool = True
) -> Generator[PlanningResult, None, None]:
    output_file = get_output_file(scenario, "anytime")
    params = {"warm_up_plan": scenario.plan} if use_warm_up else {}
    with (
        open(output_file, "w", encoding="utf-8") as output_stream,
        AnytimePlanner(name="aries", params=params) as planner,
    ):
        planner.skip_checks = True
        for idx, solution in enumerate(
            planner.get_solutions(
                scenario.problem,
                timeout=scenario.timeout,
                output_stream=output_stream,
            )
        ):
            yield PlanningResult.from_upf(scenario.problem, solution, idx)


@contextlib.contextmanager
def subtest(name: str):
    print(f"🧪 {name}", end=" ")
    yield
    print("[\033[32m✓\033[0m]")


class TestAriesWarmUp:
    def setup(self):
        os.environ["UP_ARIES_COMPILE_TARGET"] = "release"
        os.environ["ARIES_UP_ASSUME_REALS_ARE_INTS"] = "true"
        print("\n        STATUS                  QUALITY         TIME")

    @pytest.fixture(autouse=True, scope="function")
    def fixture_method(self):
        self.setup()
        yield


class TestAriesStrictWarmUp(TestAriesWarmUp):
    def setup(self):
        super().setup()
        os.environ["ARIES_LCP_SYMMETRY_BREAKING"] = "simple"
        os.environ["ARIES_WARM_UP"] = "strict"

    def test_oneshot(self, scenario: WarmUpScenario):
        result = oneshot_planning(scenario)

        with subtest("Should returns exactly the same plan"):
            assert str(result.plan) == str(scenario.plan), "Not the same plan"
            assert result.quality == scenario.quality, "Not the same quality"

    def test_anytime(self, scenario: WarmUpScenario):
        results = list(anytime_planning(scenario))

        with subtest("The first plan should be exactly the same"):
            first_result = results[0]
            assert str(first_result.plan) == str(scenario.plan), "Not the same first plan"
            assert first_result.quality == scenario.quality, "Not the same first quality"

        with subtest("The plan is improved over time"):
            best = scenario.quality + 0.1
            for idx, result in enumerate(results):
                if result.status != PlanGenerationResultStatus.INTERMEDIATE:
                    continue
                assert result.quality is not None, f"Quality is None at {idx}"
                assert result.quality < best, f"Quality is not improved at {idx}"
                best = result.quality
            assert best is not None, "Best quality is None"

        with subtest("The last result should have a plan"):
            last_result = results[-1]
            assert last_result.plan is not None, "Last plan is None"
            assert last_result.quality is not None, "Last quality is None"


class TestAriesCausalWarmUp(TestAriesWarmUp):
    def setup(self):
        super().setup()
        os.environ["ARIES_LCP_SYMMETRY_BREAKING"] = "psp"
        os.environ["ARIES_WARM_UP"] = "causal"
        os.environ["ARIES_USELESS_SUPPORTS"]="false"

    def test_oneshot(self, scenario: WarmUpScenario):
        result = oneshot_planning(scenario)

        with subtest("Should returns a plan with at least the same quality"):
            assert result.quality is not None, "Quality is None"
            assert result.quality <= scenario.quality, "Quality is not improved"

    def test_anytime(self, scenario: WarmUpScenario):
        results = list(anytime_planning(scenario))

        with subtest("The first plan should have at least the same quality"):
            first_result = results[0]
            assert first_result.quality is not None, "First quality is None"
            assert first_result.quality <= scenario.quality, "First quality is not improved"

        with subtest("The plan is improved over time"):
            best = scenario.quality + 0.1
            for idx, result in enumerate(results):
                if result.status != PlanGenerationResultStatus.INTERMEDIATE:
                    continue
                assert result.quality is not None, f"Quality is None at {idx}"
                assert result.quality < best, f"Quality is not improved at {idx}"
                best = result.quality
            assert best is not None, "Best quality is None"

        with subtest("The last result should have a plan"):
            last_result = results[-1]
            assert last_result.plan is not None, "Last plan is None"
            assert last_result.quality is not None, "Last quality is None"
