from unified_planning.shortcuts import *
from unified_planning.model.htn import *

#sys.path.insert(0, 'unified_planning')

Location = UserType("Location")
objects = [
    Object(f"l{i}", Location) for i in range(5)
]
t1 = Task("t1")
t2 = Task("t2")
t3 = Task("t3")

actions = [InstantaneousAction(f"a{i}") for i in range(10)]

def base():
    pb = HierarchicalProblem()
    pb.add_objects(objects)
    for action in actions:
        pb.add_action(action)
    for task in [t1, t2, t3]:
        pb.add_task(task)
    pb.task_network.add_subtask(t1)
    return pb

def add_method(pb, name, task, *subtasks):
    m = Method(name)
    m.set_task(task)
    subtasks = [m.add_subtask(st) for st in subtasks]
    m.set_ordered(*subtasks)
    pb.add_method(m)

def set_costs(pb, *costs):
    cost_map = {}
    print(actions)
    print(costs)
    print(list(zip(actions, costs)))
    for action, cost in zip(actions, costs):
        cost_map[action] = Int(cost)
    pb.add_quality_metric(up.model.metrics.MinimizeActionCosts(cost_map))

def problems():
    problems = []

    def export(pb, optimum, *costs):
        clone = pb.clone()
        set_costs(clone, *costs)
        problems.append((clone, optimum))

    pb = base()
    add_method(pb, "m11", t1, actions[0], actions[1])
    add_method(pb, "m12", t1, actions[2])

    export(pb, 3, 2, 2, 3)
    export(pb, 4, 2, 2, 4)
    export(pb, 4, 2, 2, 5)

    pb = base()
    add_method(pb, "m11", t1, t2, t3)
    add_method(pb, "m21", t2, actions[0], actions[1])
    add_method(pb, "m31", t3, actions[2], actions[3])
    add_method(pb, "m32", t3, actions[4], actions[5], actions[6])
    export(pb, 4,    1, 1,   1, 1,   1, 1, 1)
    export(pb, 3,    1, 1,   1, 1,   0, 0, 1)
    export(pb, 5,    1, 1,   1, 10,   1, 1, 1)
    export(pb, 13,   1, 1,   1, 10,   1, 10, 1)
    export(pb, 202,   1, 1,   100, 100,   100, 100, 100)
    return problems



#
# def go():
#     htn = HierarchicalProblem()
#
#
#     l1 = htn.add_object("l1", Location)
#     l2 = htn.add_object("l2", Location)
#     l3 = htn.add_object("l3", Location)
#     l4 = htn.add_object("l4", Location)
#
#     loc = htn.add_fluent("loc", Location)
#
#     connected = Fluent("connected", l1=Location, l2=Location)
#     htn.add_fluent(connected, default_initial_value=False)
#     htn.set_initial_value(connected(l1, l2), True)
#     htn.set_initial_value(connected(l2, l3), True)
#     htn.set_initial_value(connected(l3, l4), True)
#     htn.set_initial_value(connected(l4, l3), True)
#     htn.set_initial_value(connected(l3, l2), True)
#     htn.set_initial_value(connected(l2, l1), True)
#
#     move = InstantaneousAction("move", l_from=Location, l_to=Location)
#     l_from = move.parameter('l_from')
#     l_to = move.parameter('l_to')
#     move.add_precondition(Equals(loc, l_from))
#     move.add_precondition(connected(l_from, l_to))
#     move.add_effect(loc, l_to)
#     htn.add_action(move)
#
#     go = htn.add_task("go", target=Location)
#
#     go_noop = Method("go-noop", target=Location)
#     go_noop.set_task(go)
#     target = go_noop.parameter("target")
#     go_noop.add_precondition(Equals(loc, target))
#     htn.add_method(go_noop)
#
#     go_recursive = Method("go-recursive", source=Location, inter=Location, target=Location)
#     go_recursive.set_task(go, go_recursive.parameter("target"))
#     source = go_recursive.parameter("source")
#     inter = go_recursive.parameter("inter")
#     target = go_recursive.parameter("target")
#     go_recursive.add_precondition(Equals(loc, source))
#     go_recursive.add_precondition(connected(source, inter))
#     t1 = go_recursive.add_subtask(move, source, inter)
#     t2 = go_recursive.add_subtask(go, target)
#     go_recursive.set_ordered(t1, t2)
#     htn.add_method(go_recursive)
#
#     go1 = htn.task_network.add_subtask(go, l4)
#     final_loc = htn.task_network.add_variable("final_loc", Location)
#     go2 = htn.task_network.add_subtask(go, final_loc)
#     htn.task_network.add_constraint(Or(Equals(final_loc, l1),
#                                        Equals(final_loc, l2)))
#     htn.task_network.set_strictly_before(go1, go2)
#
#     htn.set_initial_value(loc, l1)
#     plan = up.plans.SequentialPlan([
#         up.plans.ActionInstance(move, (ObjectExp(l1), ObjectExp(l2))),
#         up.plans.ActionInstance(move, (ObjectExp(l2), ObjectExp(l3))),
#         up.plans.ActionInstance(move, (ObjectExp(l3), ObjectExp(l4))),
#         up.plans.ActionInstance(move, (ObjectExp(l4), ObjectExp(l3))),
#         up.plans.ActionInstance(move, (ObjectExp(l3), ObjectExp(l2))),
#     ])
#     htn_go = Example(problem=htn, plan=plan)
#
#     problems['htn-go'] = htn_go
#
#     return problems

if __name__ == "__main__":
    for pb, cost in problems():
        print(pb)
