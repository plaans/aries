#!/usr/bin/env python3
"""
Solve all PDDL problems in planning/problems/upf using APE.

For each problem.pddl found, reads max_depth.txt and solves with ape solve-finite.
Can export solvable problems to ci/problems.toml for CI testing.
"""

import argparse
import sys
import tomllib
from pathlib import Path

from aries_utils import ApeRunner, validate_plan


def main():
    parser = argparse.ArgumentParser(
        description="Solve all PDDL problems in planning/problems/upf"
    )
    parser.add_argument(
        "--limit",
        type=int,
        help="Maximum number of problems to solve (for testing)",
    )
    parser.add_argument(
        "-t",
        "--timeout",
        type=int,
        default=90,
        help="Timeout per problem in seconds (default: 90)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Show APE output for each solve",
    )
    parser.add_argument(
        "--export",
        type=str,
        metavar="FILE",
        help="Export solvable problems to TOML file (e.g., ci/problems.toml)",
    )
    parser.add_argument(
        "--from-toml",
        type=str,
        metavar="FILE",
        help="Solve problems listed in TOML file (e.g., ci/problems.toml)",
    )
    args = parser.parse_args()

    # Configuration
    timeout = args.timeout

    # Initialize APE runner (builds APE automatically with ci profile)
    ape = ApeRunner()

    # Get problems either from TOML file or by scanning filesystem
    if args.from_toml:
        problems_data = load_problems_from_toml(Path(args.from_toml))
        if args.limit:
            problems_data = problems_data[: args.limit]
            print(f"Limited to first {args.limit} problems")
        print(f"Loaded {len(problems_data)} problems from {args.from_toml}\n")
    else:
        # Find all problem.pddl files
        base_dir = Path("planning/problems/upf")
        problem_files = sorted(base_dir.rglob("problem.pddl"))

        # Apply limit if specified
        if args.limit:
            problem_files = problem_files[: args.limit]
            print(f"Limited to first {args.limit} problems")

        # Convert to problems_data format
        problems_data = []
        for pf in problem_files:
            max_depth_file = pf.parent / "max_depth.txt"
            if max_depth_file.exists():
                max_depth = int(max_depth_file.read_text().strip())
                problems_data.append(
                    {"path": str(pf), "max_depth": max_depth, "name": pf.parent.name}
                )

        print(f"Found {len(problems_data)} problems to solve\n")

    solved = 0
    failed = []
    solvable_problems = []

    for idx, problem_data in enumerate(problems_data, 1):
        problem_file = Path(problem_data["path"])
        max_depth = problem_data["max_depth"]

        print(f"[{idx}/{len(problems_data)}] Problem: {problem_file}")
        print(f"  Max depth: {max_depth}")

        try:
            out = Path("/tmp/plan")
            # Solve with APE
            result = ape.run(
                "solve-finite",
                str(problem_file),
                "-m",
                str(max_depth),
                "-w",
                str(out),
                timeout=timeout,
                check=True,
            )

            # Check if plan was found
            if "Found solution" in result.stdout:
                print("  ✓ Solved")
                if args.verbose:
                    # Show first few lines of output
                    lines = result.stdout.split("\n")[:10]
                    for line in lines:
                        print(f"    {line}")

                validation = validate_plan(
                    ape.find_domain(problem_file), problem_file, out
                )
                if not validation.is_valid:
                    print("APE returned an invalid plan")
                    exit(1)
                solved += 1

                # Track solvable problem
                solvable_problems.append(
                    {
                        "path": str(problem_file),
                        "max_depth": int(max_depth),
                    }
                )
            else:
                print("  ⚠️  No plan found (may be unsolvable or timeout)")
                if args.verbose:
                    print(result.stdout)

        except Exception as e:
            print(f"  ✗ Failed: {e}")
            failed.append((problem_file, str(e)))
            # Continue to next problem instead of exiting

        print()

    # Summary
    print("=" * 60)
    print("Summary:")
    print(f"  Total:  {len(problems_data)}")
    print(f"  Solved: {solved}")
    print(f"  Failed: {len(failed)}")
    print()

    # Export solvable problems if requested
    if args.export:
        export_path = Path(args.export)
        write_problems_toml(export_path, solvable_problems, timeout)
        print(f"✓ Exported {len(solvable_problems)} solvable problems to {export_path}")
        print()

    if failed:
        print("Failed problems:")
        for problem_file, error in failed:
            print(f"  - {problem_file}")
            print(f"    Error: {error}")
        return 1

    print("All problems solved successfully! ✓")
    return 0


def load_problems_from_toml(toml_path: Path) -> list:
    """Load problems from a TOML file."""
    with toml_path.open("rb") as f:
        data = tomllib.load(f)

    problems = []
    for problem in data.get("problems", []):
        problems.append(
            {
                "name": problem.get("name", ""),
                "path": problem["path"],
                "max_depth": problem["max_depth"],
            }
        )
    return problems


def write_problems_toml(output_path: Path, problems: list, timeout: int):
    """Write solvable problems to a TOML file."""

    with output_path.open("w") as f:
        f.write("# Solvable problems for CI testing\n")
        f.write("# Generated by: python ci/ape-solve.py --export ci/problems.toml\n")
        f.write(f"# Timeout used: {timeout}s\n")
        f.write("\n")

        for problem in problems:
            # Extract problem name from path
            problem_path = Path(problem["path"])
            problem_name = problem_path.parent.name

            f.write("[[problems]]\n")
            f.write(f'name = "{problem_name}"\n')
            f.write(f'path = "{problem["path"]}"\n')
            f.write(f"max_depth = {problem['max_depth']}\n")
            f.write("\n")


if __name__ == "__main__":
    sys.exit(main())
