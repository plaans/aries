#!/usr/bin/env python3

# pylint: disable=missing-function-docstring, missing-module-docstring, missing-class-docstring
# pylint: disable=too-few-public-methods, redefined-outer-name

import os
from pathlib import Path

import pytest
from unified_planning.io.pddl_reader import PDDLReader
from unified_planning.plans.plan import Plan
from unified_planning.shortcuts import OneshotPlanner, Problem


@pytest.fixture
def domain_file() -> Path:
    return Path(__file__).parent / "fixtures/warm_up/domain.pddl"


@pytest.fixture
def problem_file() -> Path:
    return Path(__file__).parent / "fixtures/warm_up/problem.pddl"


@pytest.fixture
def plan_file() -> Path:
    return Path(__file__).parent / "fixtures/warm_up/plan_76.txt"


@pytest.fixture
def problem(domain_file: Path, problem_file: Path) -> Problem:
    return PDDLReader().parse_problem(domain_file, problem_file)


@pytest.fixture
def plan(plan_file: Path, problem: Problem) -> Plan:
    return PDDLReader().parse_plan(problem, plan_file)


class TestAriesWarmUp:
    def test_oneshot_with_warm_up_returns_same_plan(self, problem: Problem, plan: Plan):
        os.environ["ARIES_UP_ASSUME_REALS_ARE_INTS"] = "true"
        os.environ["ARIES_LCP_SYMMETRY_BREAKING"] = "simple"
        with OneshotPlanner(name="aries", params={"warm_up_plan": plan}) as planner:
            planner.skip_checks = True
            result = planner.solve(problem)
        assert str(result.plan) == str(plan)
