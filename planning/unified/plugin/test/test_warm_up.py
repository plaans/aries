#!/usr/bin/env python3

# pylint: disable=missing-function-docstring, missing-module-docstring, missing-class-docstring
# pylint: disable=too-few-public-methods, redefined-outer-name

from dataclasses import dataclass
import os
from pathlib import Path
from typing import Generator

import pytest
from unified_planning.io.pddl_reader import PDDLReader
from unified_planning.plans.plan import Plan
from unified_planning.shortcuts import OneshotPlanner, Problem


@dataclass(frozen=True)
class WarmUpScenario:
    uid: str
    problem: Problem
    plan: Plan

    def __iter__(self):
        return iter((self.problem, self.plan))


def _scenarios() -> Generator[WarmUpScenario, None, None]:
    fixtures_dir = Path(__file__).parent / "fixtures/warm_up"
    for domain_dir in fixtures_dir.iterdir():
        if not domain_dir.is_dir():
            continue
        domain_file = domain_dir / "domain.pddl"
        problem_file = domain_dir / "problem.pddl"
        for plan_file in domain_dir.glob("plan_*.txt"):
            uid = f"{domain_dir.name}/{plan_file.stem}"
            problem = PDDLReader().parse_problem(domain_file, problem_file)
            plan = PDDLReader().parse_plan(problem, plan_file)
            yield WarmUpScenario(uid=uid, problem=problem, plan=plan)


@pytest.mark.parametrize("scenario", _scenarios(), ids=lambda tc: tc.uid)
class TestAriesWarmUp:
    def test_oneshot_with_warm_up_returns_same_plan(self, scenario: WarmUpScenario):
        problem, plan = scenario
        os.environ["ARIES_UP_ASSUME_REALS_ARE_INTS"] = "true"
        os.environ["ARIES_LCP_SYMMETRY_BREAKING"] = "simple"
        with OneshotPlanner(name="aries", params={"warm_up_plan": plan}) as planner:
            planner.skip_checks = True
            result = planner.solve(problem)
        assert str(result.plan) == str(plan)
