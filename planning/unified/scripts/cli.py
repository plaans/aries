#!/usr/bin/python3

import sys
import argparse

from unified_planning.shortcuts import *
from up_test_cases.report import *

from unified_planning.grpc.proto_reader import ProtobufReader
from unified_planning.grpc.proto_writer import ProtobufWriter
import unified_planning.grpc.generated.unified_planning_pb2 as proto

parser = argparse.ArgumentParser(
    prog='aries-up-cli',
    description='Utils for working with aries and the unified planning library')
parser.add_argument("-s", "--solver", default="aries", )
parser.add_argument("-m", "--mode", default="solve")
parser.add_argument("-o", "--outfile", default="/tmp/problem.upp")
parser.add_argument("--timeout", default="1800")
parser.add_argument('-f', '--from-file', action='store_true')
parser.add_argument("problem_name")


args = parser.parse_args()
print(args)


packages = ["builtin", "unified_planning.test", "up_aries_tests"]


if args.from_file:
    # reads from a protobuf file
    with open(args.problem_name, "rb") as file:
        content = file.read()
        pb_msg = proto.Problem()
        pb_msg.ParseFromString(content)
    reader = ProtobufReader()
    problem = reader.convert(pb_msg)
else:
    problem_test_cases = get_test_cases_from_packages(packages)
    test_case = problem_test_cases[args.problem_name]
    problem = test_case.problem

if args.mode == "solve":
    print("SOLVING")

    print(problem.kind)
    print(problem)

    plan = None
    try:
        with AnytimePlanner(name=args.solver) as planner:
            for r in planner.get_solutions(problem, timeout=float(args.timeout), output_stream=sys.stdout):
                print(r)
                plan = r.plan
                print("\n===================\n")
    except AssertionError:
        with OneshotPlanner(name=args.solver) as planner:
            result = planner.solve(problem, output_stream=sys.stdout)
            plan = result.plan
            print(result)

    with PlanValidator(problem_kind=problem.kind) as validator:
        val_result = validator.validate(problem, plan)
        print(val_result)

elif args.mode == "dump":
    print(problem)
    print(f"Dumping problem to {args.outfile}")
    print(test_case.problem)

    writer = ProtobufWriter()
    msg = writer.convert(problem)
    with open(args.outfile, "wb") as file:
        file.write(msg.SerializeToString())

elif args.mode == "read":
    # reads from a protobuf file
    with open(args.outfile, "rb") as file:
        content = file.read()
        pb_msg = proto.Problem()
        pb_msg.ParseFromString(content)
    reader = ProtobufReader()
    pb = reader.convert(pb_msg)
    print(pb)
