"""Tests of the in-process engines that are not covered by the unified-planning test cases (report.py)."""

import io
import os
from pathlib import Path

# environment parameters of the solver are read once per process, so they must be set before the first resolution
os.environ["ARIES_WARM_UP"] = "strict"
# as in the warm-up tests of the gRPC integration, strict warm-up is not supported with the default symmetry breaking
os.environ["ARIES_LCP_SYMMETRY_BREAKING"] = "simple"
os.environ["ARIES_UP_ASSUME_REALS_ARE_INTS"] = "true"

import pytest
from unified_planning.engines import PlanGenerationResultStatus
from unified_planning.exceptions import UPUsageError
from unified_planning.io import PDDLReader
from unified_planning.model.metrics import MinimizeSequentialPlanLength
from unified_planning.plans import HierarchicalPlan, Schedule, SequentialPlan
from unified_planning.test.examples import get_example_problems

from up_aries_pyo3 import AriesPyO3, AriesPyO3Opt

WARM_UP_FIXTURES = (
    Path(__file__).parents[2] / "unified" / "plugin" / "test" / "fixtures" / "warm_up"
)
SOLVED = (
    PlanGenerationResultStatus.SOLVED_SATISFICING,
    PlanGenerationResultStatus.SOLVED_OPTIMALLY,
)


@pytest.fixture(scope="module")
def examples():
    return get_example_problems()


def _warm_up_scenario(name: str):
    folder = WARM_UP_FIXTURES / name
    problem = PDDLReader().parse_problem(folder / "domain.pddl", folder / "problem.pddl")
    problem.clear_quality_metrics()
    problem.add_quality_metric(MinimizeSequentialPlanLength())
    plan_file = next(folder.glob("plan_*.txt"))
    return problem, PDDLReader().parse_plan(problem, plan_file)


def test_unknown_parameter():
    with pytest.raises(UPUsageError):
        AriesPyO3(unknown_param=1)


def test_unknown_strategy(examples):
    with pytest.raises(ValueError):
        AriesPyO3(strategies="unknown").solve(examples["basic"].problem)


def test_output_stream(examples, capfd):
    output = io.StringIO()
    result = AriesPyO3().solve(examples["basic"].problem, output_stream=output)
    assert result.status in SOLVED
    assert "SOLUTION FOUND" in output.getvalue()
    # without an output stream, the output of the solver is discarded
    AriesPyO3().solve(examples["basic"].problem)
    assert "SOLUTION FOUND" not in capfd.readouterr().out


def test_plan_kinds(examples):
    # also checks that problems with different epsilons (time scales) can be solved in the same process
    for name, kind in [
        ("basic", SequentialPlan),
        ("sched:basic", Schedule),
        ("htn-go", HierarchicalPlan),
        ("basic", SequentialPlan),
    ]:
        result = AriesPyO3().solve(examples[name].problem)
        assert result.status in SOLVED, name
        assert isinstance(result.plan, kind), name


def test_warm_start():
    problem, plan = _warm_up_scenario("simple")
    result = AriesPyO3().solve(problem, warm_start_plan=plan)
    assert result.status in SOLVED
    assert str(result.plan) == str(plan)


def test_anytime_improves():
    problem, plan = _warm_up_scenario("simple")
    results = list(AriesPyO3().get_solutions(problem, timeout=10, warm_start_plan=plan))
    assert str(results[0].plan) == str(plan)
    lengths = [
        len(r.plan.actions)
        for r in results
        if r.status == PlanGenerationResultStatus.INTERMEDIATE
    ]
    assert lengths == sorted(lengths, reverse=True) and len(set(lengths)) == len(lengths)
    assert results[-1].status != PlanGenerationResultStatus.INTERMEDIATE
    assert results[-1].plan is not None


def test_opt(examples):
    result = AriesPyO3Opt().solve(examples["sched:flexible-jobshop"].problem, timeout=60)
    assert result.status == PlanGenerationResultStatus.SOLVED_OPTIMALLY
