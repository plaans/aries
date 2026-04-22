#!/usr/bin/env python3
"""
Python version of ape-val.sh

Runs APE's validator on all plan files in planning/problems/upf
"""

import os
import sys
from pathlib import Path

from aries_utils import ApeRunner, validate_plan


def main():
    # Path to validators (defaults)
    pddl_val = os.getenv("PDDL_VAL", "planning/ext/val-pddl")

    # Time allowed for each run (defaults to 5s)
    timeout_str = os.getenv("TIMEOUT", "5s")
    timeout = int(timeout_str.rstrip("s"))

    # Initialize APE runner (builds APE automatically with ci profile)
    ape = ApeRunner()

    # Find all plan files
    plan_files = sorted(Path("planning/problems/upf").rglob("*.plan"))

    for plan_file in plan_files:
        print()
        print(f"Plan: {plan_file}")

        try:
            # Find problem and domain files using APE
            problem_file = ape.find_problem(plan_file)
            domain_file = ape.find_domain(problem_file)

            # Validate with VAL and extract plan quality
            result = validate_plan(domain_file, problem_file, plan_file, pddl_val)

            if not result.is_valid:
                print(f"\n{'=' * 60}")
                print("VALIDATION FAILED: VAL reports plan is invalid")
                print(f"{'=' * 60}")
                print(f"\nCommand: {pddl_val} {domain_file} {problem_file} {plan_file}")
                print("\nOutput:")
                print(result.stdout)
                if result.stderr:
                    print("\nStderr:")
                    print(result.stderr)
                print(f"\nReturn code: {result.returncode}")
                print("\nTo reproduce:")
                print(f"  {pddl_val} {domain_file} {problem_file} {plan_file}")
                sys.exit(1)

            plan_quality = result.plan_quality
            assert plan_quality is not None
            print(f"VAL plan quality: {plan_quality}")

            # Validate with APE (error reporting handled by ApeRunner)
            if not ape.validate(plan_file, plan_quality, timeout):
                sys.exit(1)

            # Run plan optimizer (error reporting handled by ApeRunner)
            if not ape.optimize_plan(
                plan_file, ["action-presence", "start-time"], timeout
            ):
                sys.exit(1)

        except Exception as e:
            print(f"\n{'=' * 60}")
            print("ERROR: Command failed")
            print(f"{'=' * 60}")
            print(f"\nError: {e}")
            sys.exit(1)


if __name__ == "__main__":
    main()
