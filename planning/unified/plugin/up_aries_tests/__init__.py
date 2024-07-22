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


def test_half_bounded_int_param():
    problem = Problem()
    problem.add_action(InstantaneousAction("a", k=IntType(lower_bound=0)))
    problem.add_action(InstantaneousAction("b", k=IntType(upper_bound=10)))
    return TestCase(problem=problem, solvable=True)
