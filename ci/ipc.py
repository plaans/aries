import os
from pathlib import Path
import re
import subprocess  # nosec: B404
import sys
import time
from typing import Dict, List, Optional, Tuple

from unified_planning.engines.results import PlanGenerationResultStatus as Status
from unified_planning.io.pddl_reader import PDDLReader
from unified_planning.plans import Plan
from unified_planning.shortcuts import OneshotPlanner


# ============================================================================ #
#                                   Constants                                  #
# ============================================================================ #


ESC_TABLE = {
    "black": 30,
    "red": 31,
    "green": 32,
    "yellow": 33,
    "blue": 34,
    "purple": 35,
    "cyan": 36,
    "white": 37,
    "Black": 40,
    "Red": 41,
    "Green": 42,
    "Yellow": 43,
    "Blue": 44,
    "Purple": 45,
    "Cyan": 46,
    "White": 47,
    "bold": 1,
    "light": 2,
    "blink": 5,
    "invert": 7,
}
SLOW_THRESHOLD = 5
VALID_STATUS = {Status.SOLVED_SATISFICING, Status.SOLVED_OPTIMALLY}

IS_GITHUB_ACTIONS = os.getenv("GITHUB_ACTIONS") == "true"


# ============================================================================ #
#                                     Utils                                    #
# ============================================================================ #


def write(text: str = "", **markup: bool) -> None:
    """Write text with ANSI escape sequences for formatting."""
    for name in markup:
        if name not in ESC_TABLE:
            raise ValueError(f"Unknown markup: {name}")
    esc = [ESC_TABLE[name] for name, value in markup.items() if value]
    if esc:
        text = "".join(f"\x1b[{cod}m" for cod in esc) + text + "\x1b[0m"
    print(text, end="")


def write_line(text: str = "", **markup: bool) -> None:
    """Write a line of text with ANSI escape sequences for formatting."""
    write(text, **markup)
    if not text.endswith("\n"):
        print()


# pylint: disable=too-many-arguments
def separator(
    sep: str = "=",
    title: Optional[str] = None,
    width: int = 80,
    before: str = "\n",
    after: str = "\n",
    github_group: bool = False,
    **markup: bool,
) -> None:
    """Print a separator line with optional title."""
    if title is None:
        line = sep * (width // len(sep))
    else:
        n = max((width - len(title) - 2) // (2 * len(sep)), 1)
        fill = sep * n
        line = f"{fill} {title} {fill}"
    if len(line) + len(sep.rstrip()) <= width:
        line += sep.rstrip()
    if github_group and IS_GITHUB_ACTIONS:
        write_line(f"::group::{title}")
    write_line(f"{before}{line}{after}", **markup)


def big_separator(
    sep: str = "=",
    title: Optional[str] = None,
    width: int = 80,
    **markup: bool,
):
    """Print a big separator line with optional title."""
    separator(sep=sep, title=None, width=width, before="\n", after="", **markup)
    separator(sep=" ", title=title, width=width, before="", after="", **markup)
    separator(sep=sep, title=None, width=width, before="", after="\n", **markup)


# ============================================================================ #
#                                  Validators                                  #
# ============================================================================ #


def extract_plan_for_val(plan: Plan, dom: Path, pb: Path, out: Path) -> None:
    """Reformat a plan to be validated by Val."""

    domain_content = dom.read_text()
    problem_content = pb.read_text()

    def fix_upf_shit(name: str) -> str:
        name_alt = name.replace("-", "_")
        if name in domain_content or name in problem_content:
            return name
        if name_alt in domain_content or name_alt in problem_content:
            return name_alt
        raise ValueError(f"Name {name} not found in domain or problem")

    def format_line(line: str) -> str:
        if line.startswith(";"):
            return line

        regex = r"^\s*(?:(?P<time>\d+\.\d+):\s?)?(?P<name>[\w-]+)(?:\((?P<params>[\w, -]+)\))?\s?(?:\[(?P<duration>\d+\.\d+)\])?"  # pylint:disable=line-too-long # noqa: E501
        if not (match := re.match(regex, line)):
            return line
        groups = match.groupdict()

        new_line = ""
        if groups["time"]:
            new_line += f"{groups['time']}: "
        new_line += "("
        new_line += fix_upf_shit(groups["name"])
        if groups["params"]:
            new_line += " " + " ".join(map(fix_upf_shit, groups["params"].split(", ")))
        new_line += ")"
        if groups["duration"]:
            new_line += f" [{groups['duration']}]"
        return new_line

    lines = str(plan).splitlines()
    out.write_text("\n".join(map(format_line, lines[1:])))


def extract_max_depth(log_file: Path) -> int:
    """Extract the max depth used by the planner from the logs."""
    depth = 0
    for line in log_file.read_text().splitlines():
        if "Solving with depth" in line:
            depth = int(line.split("Solving with depth")[-1])
    return depth


def validate_plan_with_val(pb: Path, dom: Path, plan: Path) -> bool:
    """Validate a plan using Val."""
    ext = "pddl"
    if ":hierarchy" in dom.read_text():
        ext = "hddl"
    cmd = (
        f"./planning/ext/val-{ext} "
        f"{dom.as_posix()} {pb.as_posix()} {plan.as_posix()}"
    )
    return (
        subprocess.run(
            cmd,
            shell=True,  # nosec: B602
            check=False,
            capture_output=True,
        ).returncode
        == 0
    )


# ============================================================================ #
#                                     Main                                     #
# ============================================================================ #


pb_folders = Path(__file__).parent.parent / "planning/problems/upf"
problems = sorted(pb_folders.iterdir(), key=lambda f: f.stem)

if len(sys.argv) > 1:
    problems = [pb for pb in problems if pb.stem in sys.argv[1:]]

last_results: Dict[str, Tuple[str, float]] = {}
last_results_file = Path("/tmp/aries_ci_ipc_last_best_results.csv")  # nosec: B108
if last_results_file.exists():
    lines = last_results_file.read_text(encoding="utf-8").splitlines()
    for _line in lines:
        pb, s, t = _line.split(",")
        last_results[pb] = (s, float(t))

valid: List[str] = []
invalid: List[str] = []
unsolved: List[Tuple[str, str]] = []
update_depth: List[str] = []

timing: Dict[str, float] = {}
status: Dict[str, str] = {}

try:
    for i, folder in enumerate(problems):
        separator(
            title=f"Problem {folder.stem} ({i + 1}/{len(problems)})",
            github_group=True,
            bold=True,
            blue=True,
        )

        out_file = f"/tmp/aries-{folder.stem}.log"  # nosec: B108

        try:
            domain = folder / "domain.pddl"
            problem = folder / "problem.pddl"
            upf_pb = PDDLReader().parse_problem(domain, problem)
            has_max_depth = (folder / "max_depth.txt").exists() or IS_GITHUB_ACTIONS
            if has_max_depth:
                max_depth = int((folder / "max_depth.txt").read_text())
            else:
                max_depth = 100  # pylint: disable=invalid-name

            with open(out_file, mode="w+", encoding="utf-8") as output:
                write_line(f"Output log: {output.name}\n")

                with OneshotPlanner(
                    name="aries",
                    params={"max-depth": max_depth},
                ) as planner:
                    # Problem Kind
                    write_line("Problem Kind", cyan=True)
                    write_line(str(upf_pb.kind), light=True)
                    write_line()

                    # Unsupported features
                    unsupported = upf_pb.kind.features.difference(
                        planner.supported_kind().features
                    )
                    if unsupported:
                        write_line("Unsupported Features", cyan=True)
                        write_line("\n".join(unsupported), light=True)
                        write_line()

                    # Resolution
                    start = time.time()
                    result = planner.solve(upf_pb, output_stream=output)
                    write_line("Resolution status", cyan=True)
                    timing[folder.stem] = time.time() - start
                    write_line(f"Elapsed time: {timing[folder.stem]:.3f} s", light=True)
                    write_line(f"Status: {result.status}", light=True)

                    # Update max depth
                    if not has_max_depth:
                        # pylint: disable=invalid-name
                        max_depth = extract_max_depth(Path(output.name))
                        (folder / "max_depth.txt").write_text(str(max_depth))
                        update_depth.append(folder.stem)

                    # Solved problem
                    if result.status in VALID_STATUS:
                        write_line("\nValidating plan by Val", cyan=True)
                        out_path = Path(output.name)
                        extract_plan_for_val(result.plan, domain, problem, out_path)
                        if validate_plan_with_val(problem, domain, out_path):
                            write_line("Plan is valid", bold=True, green=True)
                            valid.append(folder.stem)
                            status[folder.stem] = "valid"
                        else:
                            write_line("Plan is invalid", bold=True, red=True)
                            invalid.append(folder.stem)
                            status[folder.stem] = "invalid"

                    # Unsolved problem
                    else:
                        write_line(
                            Path(out_file).read_text(encoding="utf-8"), yellow=True
                        )
                        unsolved.append((folder.stem, result.status.name))
                        status[folder.stem] = result.status.name.lower()
                        write_line("Unsolved problem", bold=True, red=True)

        except Exception as e:  # pylint: disable=broad-except
            unsolved.append((folder.stem, "Error"))
            status[folder.stem] = "error"
            write_line(f"Error: {e}", bold=True, red=True)
            write_line()
            write_line(Path(out_file).read_text(encoding="utf-8"), yellow=True)

        finally:
            if IS_GITHUB_ACTIONS:
                write_line("::endgroup::")

except KeyboardInterrupt:
    pass
finally:
    # Summary
    separator(title="Summary", bold=True, blue=True)

    csv_data = ""  # pylint: disable=invalid-name
    for folder in problems:
        t = min(  # type: ignore
            timing[folder.stem],
            (
                last_results[folder.stem][1]
                if folder.stem in last_results
                else float("inf")
            ),
        )
        csv_data += f"{folder.stem},{status[folder.stem]},{t}\n"
    last_results_file.write_text(csv_data, encoding="utf-8")

    if valid:
        big_separator(title=f"Valid: {len(valid)}", bold=True, green=True)
        offset = max(map(len, valid)) + 3
        for res in valid:
            time_str = f" {last_results[res][1]:.3f} -> " if res in last_results else ""
            time_str += f"{timing[res]:.3f}"
            write(f"{res:<{offset}} ")
            if res in last_results and timing[res] < last_results[res][1]:
                write_line(time_str, bold=True, green=True)
            elif (
                res in last_results
                and timing[res] - last_results[res][1] > SLOW_THRESHOLD
            ):
                write_line(time_str, red=True)
            else:
                write_line(time_str)

    slow = list(filter(lambda t: t[1] > SLOW_THRESHOLD, timing.items()))
    if slow:
        big_separator(title=f"Slow: {len(slow)}", bold=True, yellow=True)
        offset = max(map(len, map(lambda t: t[0], slow))) + 3
        for res, t in sorted(slow, key=lambda t: t[1], reverse=True):  # type: ignore
            write_line(f"{res:<{offset}} {t:.3f}")

    if invalid:
        big_separator(title=f"Invalid: {len(invalid)}", bold=True, red=True)
        for res in invalid:
            write_line(res)

    if unsolved:
        big_separator(title=f"Unsolved: {len(unsolved)}", bold=True, red=True)
        offset = max(map(len, map(lambda t: t[0], unsolved))) + 3
        for res, reason in unsolved:
            write_line(f"{res:<{offset}} {reason}")

    if update_depth:
        big_separator(title=f"Updated depth: {len(update_depth)}", bold=True)
        for res in update_depth:
            write_line(res)

    EXIT_CODE = 0 if not invalid and not unsolved else 1
    big_separator(title=f"End with code {EXIT_CODE}", bold=True)
    sys.exit(EXIT_CODE)
