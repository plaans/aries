"""Unified-planning engines for Aries, running the planner in-process through PyO3."""

import contextlib
import os
import queue
import sys
import tempfile
import threading
from fractions import Fraction
from typing import IO, Any, Callable, Dict, Iterator, List, Optional, Union

import unified_planning as up
import unified_planning.engines.mixins as mixins
from unified_planning import engines
from unified_planning.engines import (
    AnytimeGuarantee,
    LogLevel,
    LogMessage,
    PlanGenerationResult,
    PlanGenerationResultStatus,
)
from unified_planning.engines.mixins.oneshot_planner import OptimalityGuarantee
from unified_planning.exceptions import UPUsageError
from unified_planning.model.htn import HierarchicalProblem
from unified_planning.plans import (
    ActionInstance,
    HierarchicalPlan,
    Schedule,
    SequentialPlan,
    TimeTriggeredPlan,
)
from unified_planning.plans.hierarchical_plan import Decomposition, MethodInstance

from up_aries_pyo3 import _aries

_ARIES_EPSILON = Fraction(1, 10)

# Same as `_ARIES_SUPPORTED_KIND` of the `up_aries` (gRPC) integration.
_ARIES_SUPPORTED_KIND = up.model.ProblemKind(
    {
        # PROBLEM_CLASS
        "ACTION_BASED",
        "HIERARCHICAL",
        "SCHEDULING",
        # PROBLEM_TYPE
        "SIMPLE_NUMERIC_PLANNING",
        # TIME
        "CONTINUOUS_TIME",
        "DISCRETE_TIME",
        "INTERMEDIATE_CONDITIONS_AND_EFFECTS",
        "EXTERNAL_CONDITIONS_AND_EFFECTS",
        "TIMED_EFFECTS",
        "TIMED_GOALS",
        "DURATION_INEQUALITIES",
        "SELF_OVERLAPPING",
        # EXPRESSION_DURATION
        "STATIC_FLUENTS_IN_DURATIONS",
        "FLUENTS_IN_DURATIONS",
        "INT_TYPE_DURATIONS",
        # NUMBERS
        "DISCRETE_NUMBERS",
        "BOUNDED_TYPES",
        # CONDITIONS_KIND
        "NEGATIVE_CONDITIONS",
        "DISJUNCTIVE_CONDITIONS",
        "EQUALITIES",
        # EFFECTS_KIND
        "INCREASE_EFFECTS",
        "DECREASE_EFFECTS",
        "STATIC_FLUENTS_IN_BOOLEAN_ASSIGNMENTS",
        "STATIC_FLUENTS_IN_NUMERIC_ASSIGNMENTS",
        "STATIC_FLUENTS_IN_OBJECT_ASSIGNMENTS",
        "FLUENTS_IN_BOOLEAN_ASSIGNMENTS",
        "FLUENTS_IN_NUMERIC_ASSIGNMENTS",
        "FLUENTS_IN_OBJECT_ASSIGNMENTS",
        # TYPING
        "FLAT_TYPING",
        "HIERARCHICAL_TYPING",
        # FLUENTS_TYPE
        "NUMERIC_FLUENTS",
        "OBJECT_FLUENTS",
        "INT_FLUENTS",
        # PARAMETERS
        "BOOL_ACTION_PARAMETERS",
        "BOUNDED_INT_ACTION_PARAMETERS",
        "UNBOUNDED_INT_ACTION_PARAMETERS",
        # QUALITY_METRICS
        "ACTIONS_COST",
        "FINAL_VALUE",
        "MAKESPAN",
        "PLAN_LENGTH",
        "INT_NUMBERS_IN_OVERSUBSCRIPTION",
        # ACTIONS_COST_KIND
        "STATIC_FLUENTS_IN_ACTIONS_COST",
        "FLUENTS_IN_ACTIONS_COST",
        "INT_NUMBERS_IN_ACTIONS_COST",
        # HIERARCHICAL
        "METHOD_PRECONDITIONS",
        "TASK_NETWORK_CONSTRAINTS",
        "INITIAL_TASK_NETWORK_VARIABLES",
        "TASK_ORDER_TOTAL",
        "TASK_ORDER_PARTIAL",
        # INITIAL_STATE
        "UNDEFINED_INITIAL_NUMERIC",
        "UNDEFINED_INITIAL_SYMBOLIC",
        # SCHEDULING
        "OPTIONAL_ACTIVITIES",
        "SCOPED_CONSTRAINTS",
    },
    version=2,
)

_PARAMS = ("min_depth", "max_depth", "strategies")


@contextlib.contextmanager
def _redirect_output(output_stream: Optional[IO[str]]):
    """Sends what is written to file descriptors 1 and 2 (the native solver's output) to `output_stream`, or discards it."""
    # ponytail: process-wide and copied at the end, a pipe with a reader thread if live output is needed
    sys.stdout.flush()
    sys.stderr.flush()
    saved = [os.dup(1), os.dup(2)]
    with tempfile.TemporaryFile() if output_stream is not None else open(
        os.devnull, "wb"
    ) as sink:
        os.dup2(sink.fileno(), 1)
        os.dup2(sink.fileno(), 2)
        try:
            yield
        finally:
            for fd, saved_fd in enumerate(saved, start=1):
                os.dup2(saved_fd, fd)
                os.close(saved_fd)
            if output_stream is not None:
                sink.seek(0)
                output_stream.write(sink.read().decode(errors="replace"))


def _value(problem: "up.model.AbstractProblem", value: Union[bool, int, Fraction, str]):
    """Converts a value of a plan into an expression of the problem."""
    em = problem.environment.expression_manager
    if isinstance(value, bool):
        return em.Bool(value)
    if isinstance(value, int):
        return em.Int(value)
    if isinstance(value, Fraction):
        return em.Real(value)
    if problem.has_object(value):
        return em.ObjectExp(problem.object(value))
    return problem.fluent(value)


def _schedule(problem, data: Dict[str, Any]) -> Schedule:
    activities = [problem.get_activity(name) for name in data["activities"]]
    assignment = {}
    for var, val in data["variable_assignments"].items():
        if "." in var:
            activity_name, parameter_name = var.split(".")
            activity = problem.get_activity(activity_name)
            if parameter_name == "start":
                param = activity.start
            elif parameter_name == "end":
                param = activity.end
            else:
                param = activity.get_parameter(parameter_name)
        else:
            param = problem.get_variable(var)
        assignment[param] = _value(problem, val)
    return Schedule(assignment=assignment, activities=activities)


def _plan(problem: "up.model.AbstractProblem", data: Dict[str, Any]) -> "up.plans.Plan":
    """Builds a UP plan from the plain data returned by the native module."""
    if data["schedule"] is not None:
        return _schedule(problem, data["schedule"])

    actions = []
    for a in data["actions"]:
        instance = ActionInstance(
            problem.action(a["action_name"]),
            tuple(_value(problem, p) for p in a["parameters"]),
        )
        actions.append((a["id"], instance, a["start_time"], a["end_time"]))
    if all(start is not None for _, _, start, _ in actions):
        flat_plan = TimeTriggeredPlan(
            [
                (start, instance, None if end == start else end - start)
                for _, instance, start, end in actions
            ]
        )
    else:
        flat_plan = SequentialPlan(actions=[instance for _, instance, _, _ in actions])

    hierarchy = data["hierarchy"]
    if hierarchy is None:
        return flat_plan
    assert isinstance(problem, HierarchicalProblem)
    instances = {id: instance for id, instance, _, _ in actions}
    methods = {m["id"]: m for m in hierarchy["methods"]}

    def instance_with_id(id: str) -> Union[ActionInstance, MethodInstance]:
        if id not in instances:
            m = methods[id]
            instances[id] = MethodInstance(
                problem.method(m["method_name"]),
                tuple(_value(problem, p) for p in m["parameters"]),
                Decomposition(
                    {st: instance_with_id(impl) for st, impl in m["subtasks"].items()}
                ),
            )
        return instances[id]

    decomposition = Decomposition(
        {task: instance_with_id(impl) for task, impl in hierarchy["root_tasks"].items()}
    )
    return HierarchicalPlan(flat_plan, decomposition)


class _AriesPyO3Base(engines.engine.Engine, mixins.OneshotPlannerMixin):
    """Base class of the Aries planners running in-process."""

    _optimal = False

    def __init__(
        self,
        min_depth: int = 0,
        max_depth: Optional[int] = None,
        strategies: Optional[Union[str, List[str]]] = None,
        **kwargs,
    ):
        if kwargs:
            raise UPUsageError(
                f"Unknown parameters for {self.name}: {sorted(kwargs)}. Supported parameters: {list(_PARAMS)}"
            )
        engines.engine.Engine.__init__(self)
        mixins.OneshotPlannerMixin.__init__(self)
        if isinstance(strategies, str):
            strategies = [s.strip() for s in strategies.split(",") if s.strip()]
        self._solver_params = {
            "min_depth": int(min_depth),
            "max_depth": 2**32 - 1 if max_depth is None else int(max_depth),
            "strategies": list(strategies or []),
        }
        self.optimality_metric_required = False

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        return _ARIES_SUPPORTED_KIND

    def _result(
        self, problem: "up.model.AbstractProblem", data: Dict[str, Any]
    ) -> PlanGenerationResult:
        result = PlanGenerationResult(
            status=PlanGenerationResultStatus[data["status"]],
            plan=_plan(problem, data["plan"]) if data["plan"] is not None else None,
            engine_name=self.name,
            metrics=data["metrics"],
            log_messages=[LogMessage(LogLevel.ERROR, m) for m in data["log_messages"]]
            or None,
        )
        # if we have a time triggered plan and a recent version of the UP that support setting epsilon-separation,
        # send the result through an additional (in)validation to ensure it meets the minimal separation
        if isinstance(result.plan, TimeTriggeredPlan) and "correct_plan_generation_result" in dir(
            up.engines.results
        ):
            result = up.engines.results.correct_plan_generation_result(
                result, problem, _ARIES_EPSILON
            )
        return result

    def _native_solve(
        self,
        problem: "up.model.AbstractProblem",
        optimal: bool,
        timeout: Optional[float],
        warm_start_plan: Optional["up.plans.Plan"],
        on_solution: Optional[Callable[[Dict[str, Any]], None]] = None,
    ) -> Dict[str, Any]:
        return _aries.solve(
            problem,
            optimal=optimal,
            timeout=timeout,
            warm_start_plan=warm_start_plan,
            on_solution=on_solution,
            **self._solver_params,
        )

    def _solve_with_params(
        self,
        problem: "up.model.AbstractProblem",
        heuristic: Optional[Callable[["up.model.state.ROState"], Optional[float]]] = None,
        timeout: Optional[float] = None,
        output_stream: Optional[IO[str]] = None,
        warm_start_plan: Optional["up.plans.Plan"] = None,
        **kwargs,
    ) -> PlanGenerationResult:
        assert isinstance(problem, up.model.AbstractProblem)
        if heuristic is not None:
            print(
                "Warning: The aries solver does not support custom heuristic (as it is not a state-space planner)."
            )
        with _redirect_output(output_stream):
            data = self._native_solve(problem, self._optimal, timeout, warm_start_plan)
        return self._result(problem, data)

    def _solve(
        self,
        problem: "up.model.AbstractProblem",
        heuristic: Optional[Callable[["up.model.state.ROState"], Optional[float]]] = None,
        timeout: Optional[float] = None,
        output_stream: Optional[IO[str]] = None,
    ) -> PlanGenerationResult:
        return self._solve_with_params(problem, heuristic, timeout, output_stream)


class AriesPyO3(_AriesPyO3Base, mixins.AnytimePlannerMixin):
    """Aries planner, supporting oneshot and anytime planning."""

    def __init__(self, **kwargs):
        _AriesPyO3Base.__init__(self, **kwargs)
        mixins.AnytimePlannerMixin.__init__(self)

    @property
    def name(self) -> str:
        return "aries-pyo3"

    @staticmethod
    def satisfies(optimality_guarantee: OptimalityGuarantee) -> bool:
        # in general, we cannot provide optimality guarantees except for non-recursive HTNs
        return optimality_guarantee == OptimalityGuarantee.SATISFICING

    @staticmethod
    def ensures(anytime_guarantee: AnytimeGuarantee) -> bool:
        return anytime_guarantee == AnytimeGuarantee.INCREASING_QUALITY

    @staticmethod
    def supports(problem_kind: up.model.ProblemKind) -> bool:
        return problem_kind <= AriesPyO3.supported_kind()

    def _get_solutions_with_params(
        self,
        problem: "up.model.AbstractProblem",
        timeout: Optional[float] = None,
        output_stream: Optional[IO[str]] = None,
        warm_start_plan: Optional["up.plans.Plan"] = None,
        **kwargs,
    ) -> Iterator[PlanGenerationResult]:
        results: "queue.Queue[Union[Dict[str, Any], BaseException]]" = queue.Queue()

        def run():
            # the solver thread owns the redirection, as it may outlive the consumer of the results
            try:
                with _redirect_output(output_stream):
                    results.put(
                        self._native_solve(problem, True, timeout, warm_start_plan, results.put)
                    )
            except BaseException as e:  # forwarded to the consumer
                results.put(e)

        # ponytail: no cancellation in aries, the thread runs until a definitive answer or the timeout
        threading.Thread(target=run, daemon=True).start()
        while True:
            data = results.get()
            if isinstance(data, BaseException):
                raise data
            result = self._result(problem, data)
            yield result
            if result.status != PlanGenerationResultStatus.INTERMEDIATE:
                break  # definitive answer

    def _get_solutions(
        self,
        problem: "up.model.AbstractProblem",
        timeout: Optional[float] = None,
        output_stream: Optional[IO[str]] = None,
    ) -> Iterator[PlanGenerationResult]:
        return self._get_solutions_with_params(problem, timeout, output_stream)


class AriesPyO3Opt(_AriesPyO3Base):
    """Variant of Aries that guarantees the optimality of returned solutions."""

    _optimal = True

    @property
    def name(self) -> str:
        return "aries-pyo3-opt"

    @staticmethod
    def satisfies(optimality_guarantee: OptimalityGuarantee) -> bool:
        return optimality_guarantee in [
            OptimalityGuarantee.SOLVED_OPTIMALLY,
            OptimalityGuarantee.SATISFICING,
        ]

    @staticmethod
    def supported_kind() -> up.model.ProblemKind:
        kind = _ARIES_SUPPORTED_KIND.clone()
        # optimality cannot be proven for generative planning
        kind.unset_problem_class("ACTION_BASED")
        return kind

    @staticmethod
    def supports(problem_kind: up.model.ProblemKind) -> bool:
        return problem_kind <= AriesPyO3Opt.supported_kind()
