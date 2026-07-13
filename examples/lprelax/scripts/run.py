#!/usr/bin/env python3

import argparse
import sys
import tomllib
from pathlib import Path

from aries_utils import ApeRunner, validate_plan


def main():
    parser = argparse.ArgumentParser()

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
        "--report",
        type=str,
        metavar="DIR",
        help="Export json reports to folder",
    )
    parser.add_argument(
        "--from-toml",
        type=str,
        metavar="FILE",
        help="Solve problems listed in TOML file (e.g., ci/problems.toml)",
    )
    parser.add_argument(
        "--run-name",
        type=str,
    )
    args = parser.parse_args()

    run_name = args.run_name if args.run_name else "run-name"

    # Configuration
    timeout = args.timeout

    # Initialize APE runner (builds APE automatically with ci profile)
    apelprelax = ApeRunner(profile="release", bin_="ape-lprelax")

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

                if max_depth <= 1:
                    print("SKIPPED: {pf} (max_depth <= 0)")
                    continue
                max_depth = max_depth - 1

                problems_data.append(
                    {"path": str(pf), "max_depth": max_depth, "name": pf.parent.name}
                )

        print(f"Found {len(problems_data)} problems to solve\n")
        
    report = args.report

    solved = 0
    failed = []

    for idx, problem_data in enumerate(problems_data, 1):
        problem_file = Path(problem_data["path"])
        max_depth = problem_data["max_depth"]

        print(f"[{idx}/{len(problems_data)}] Problem: {problem_file}")
        print(f"  Max depth: {max_depth}")

        try:
            if report:
                result = apelprelax.run(
                    str(problem_file),
                    "-m",
                    str(max_depth),
                    "--timeout",
                    str(timeout),
                    "--report",
                    report,
                    timeout=timeout,
                    check=True,
                )
            else:
                result = apelprelax.run(
                    str(problem_file),
                    "-m",
                    str(max_depth),
                    "--timeout",
                    str(timeout),
                    timeout=timeout,
                    check=True,
                )

            # Check if plan was found
            if "UNSATISFIABLE" in result.stdout:
                print("  [V] Solved(Unsat)")
                if args.verbose:
                    # Show first few lines of output
                    lines = result.stdout.split("\n")[:10]
                    for line in lines:
                        print(f"    {line}")

                solved +=1

            else:
                print("  [X] Satisfiable or timeout")
                print(result.stdout)

        except Exception as e:
            print(f"  [X] Failed: {e}")
            failed.append((problem_file, str(e)))
            # Continue to next problem instead of exiting

        print()

    # Summary
    print("=" * 60)
    print("Summary:")
    print(f"  Total:  {len(problems_data)}")
    print(f"  Solved(Unsat): {solved}")
    print(f"  Failed: {len(failed)}")
    print()

    if report:

        import json
        json_entries = []
        report = Path(report)

        for json_file in report.glob("*.json"):
            if json_file.name == "summary.json":
                continue  # Skip the output file if it already exists

            with json_file.open("r", encoding="utf-8") as f:
                json_entries.append(json.load(f))

        # Write the combined list
        summary_dir = report / "summary"
        summary_dir.mkdir(parents=True, exist_ok=True)
        
        summary_json = summary_dir / "summary.json"
        with summary_json.open("w", encoding="utf-8") as f:
            json.dump({ "run_name": run_name, "entries": json_entries }, f, indent=2, ensure_ascii=False)
    

def load_problems_from_toml(toml_path: Path) -> list:
    """Load problems from a TOML file."""
    with toml_path.open("rb") as f:
        data = tomllib.load(f)

    problems = []
    for problem in data.get("problems", []):

        max_depth = problem["max_depth"]
        if max_depth <= 1:
            print(f"SKIPPED: {problem.get("name", "")} (max_depth <= 0)")
            continue
        max_depth = max_depth - 1

        problems.append(
            {
                "name": problem.get("name", ""),
                "path": problem["path"],
                "max_depth": max_depth,
            }
        )
    return problems


if __name__ == "__main__":
    sys.exit(main())
