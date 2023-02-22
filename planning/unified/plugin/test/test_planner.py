#!/usr/bin/env python3
import pytest
from unified_planning.engines.results import PlanGenerationResultStatus
from unified_planning.shortcuts import *
from unified_planning.test.examples import get_example_problems
from up_aries import Aries

INSTANCES = get_example_problems()

# TODO: this is a workaround while waiting for the inclusion of Aries upstream
env = up.environment.get_env()
env.factory.add_engine("aries", "up_aries", "Aries")


class TestAries:
    def test_setup(self):
        aries = Aries()

    def test_up_setup(self):
        with OneshotPlanner(name="aries") as planner:
            assert planner.name == "aries"

    @pytest.mark.parametrize(
        "instance",
        ["basic", "basic_without_negative_preconditions", "basic_nested_conjunctions"],
    )
    def test_basic_problem(self, instance):
        self._test_problem(instance)
        self._test_up_problem(instance)

    @pytest.mark.parametrize(
        "instance",
        [
            "htn-go",
            "hierarchical_blocks_world",
            "hierarchical_blocks_world_object_as_root",
            "hierarchical_blocks_world_with_object",
        ],
    )
    def test_hierarchical_problem(self, instance):
        self._test_problem(instance)
        self._test_up_problem(instance)

    @pytest.mark.parametrize("instance", ["matchcellar"])
    def test_matchcellar_problem(self, instance):
        self._test_problem(instance)
        self._test_up_problem(instance)

    def _test_problem(self, instance):
        aries = Aries()
        problem = INSTANCES[instance].problem
        result = aries.solve(problem)

        assert result is not None
        assert result.status == PlanGenerationResultStatus.SOLVED_SATISFICING

    def _test_up_problem(self, instance):
        if instance in INSTANCES:
            with OneshotPlanner(name="aries") as planner:
                problem = INSTANCES[instance].problem
                plan = planner.solve(problem)
                assert plan is not None
                assert plan.status == PlanGenerationResultStatus.SOLVED_SATISFICING
