#!/usr/bin/env python3
from collections import namedtuple

import pytest
from unified_planning.engines.results import ValidationResultStatus
from unified_planning.shortcuts import *
from unified_planning.test.examples import (
    hierarchical,
    minimals,
    realistic,
    testing_variants,
)
from up_aries import AriesVal

get_environment().factory.add_engine("aries-val", "up_aries", "AriesVal")

Example = namedtuple("Example", ["problem", "plan"])


problem_ids = []
problem_instances = []
# NOTE Does not support multi-agent problems
for module in (hierarchical, minimals, realistic, testing_variants):
    module_name = module.__name__.split(".")[-1]
    instances = module.get_example_problems()
    for instance_name, problem_instance in instances.items():
        problem_ids.append(f"{module_name} - {instance_name}")
        problem_instances.append(problem_instance)


class TestAriesVal:
    def test_setup(self):
        _aries = AriesVal()

    def test_up_setup(self):
        with PlanValidator(name="aries-val") as validator:
            assert validator.name == "aries-val"
            assert isinstance(validator, AriesVal)

    @pytest.mark.parametrize("instance", problem_instances, ids=problem_ids)
    def test_problem(self, instance):
        print("=====")
        print(instance.problem.kind)
        print("=====")
        self._test_problem(instance)
        self._test_up_problem(instance)

    def _test_problem(self, instance: Example):
        aries = AriesVal()
        problem = instance.problem
        plan = instance.plan
        result = aries.validate(problem, plan)

        assert result is not None
        assert result.status == ValidationResultStatus.VALID

    def _test_up_problem(self, instance: Example):
        with PlanValidator(name="aries-val") as validator:
            problem = instance.problem
            plan = instance.plan
            result = validator.validate(problem, plan)

            assert result is not None
            assert result.status == ValidationResultStatus.VALID
