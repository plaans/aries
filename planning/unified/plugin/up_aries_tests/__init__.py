from unified_planning.shortcuts import *
from unified_planning.test import TestCase
import unified_planning.model.scheduling as sched


def get_test_cases():
    """Creates on test case for each function starting with `test_` in the file"""
    res = {}
    gens = [(name.removeprefix('test_'), fn) for name, fn in globals().items()
            if callable(fn) and fn.__module__ == __name__ and name.startswith('test_')]

    for (name, generator) in gens:
        print(name)
        res[name] = generator()

    return res


def test_sched_bool_param():
    problem = sched.SchedulingProblem()

    a = problem.add_activity('a', 10)

    a.add_parameter('p', BoolType())

    return TestCase(problem=problem, solvable=True)


def test_action_costs():
    problem = Problem()
    pa = problem.add_fluent("pa", BoolType(), default_initial_value=False)
    pb = problem.add_fluent("pb", BoolType(), default_initial_value=False)
    a = InstantaneousAction("a")
    a.add_precondition(Not(pa))
    a.add_effect(pa, True)
    b = InstantaneousAction("b", k=IntType())
    b.add_precondition(Not(pb))
    b.add_effect(pb, True)
    costs = {a: 10, b: b.k}
    problem.add_actions([a, b])
    problem.add_quality_metric(MinimizeActionCosts(costs, 1))
    problem.add_goal(pa)
    problem.add_goal(pb)
    return TestCase(problem=problem, solvable=True)
