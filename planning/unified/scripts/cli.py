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
parser.add_argument("problem_name")


args = parser.parse_args()
print(args)


packages = ["builtin", "unified_planning.test"]
problem_test_cases = get_test_cases_from_packages(packages)


test_case = problem_test_cases[args.problem_name]

if args.mode == "solve":
    print("SOLVING")
    # with OneshotPlanner(name=args.solver) as planner:
    #     result = planner.solve(test_case.problem, output_stream=sys.stdout)
    #
    #     print(result)
    with AnytimePlanner(name=args.solver) as planner:
        for r in planner.get_solutions(test_case.problem, timeout=float(args.timeout), output_stream=sys.stdout):
            print(r)
            print("\n===================\n")

        # print(result)

elif args.mode == "dump":
    print(f"Dumping problem to {args.outfile}")
    print(test_case.problem)

    writer = ProtobufWriter()
    msg = writer.convert(test_case.problem)
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
