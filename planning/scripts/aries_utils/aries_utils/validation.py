"""Plan validation utilities."""

import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class ValidationResult:
    """Result of plan validation."""

    is_valid: bool
    plan_quality: Optional[str] = None
    stdout: str = ""
    stderr: str = ""
    returncode: int = 0


def extract_plan_quality(val_output: str) -> Optional[str]:
    """Extract plan quality from VAL output."""
    for line in val_output.split("\n"):
        if "Final value:" in line:
            parts = line.split()
            for i, part in enumerate(parts):
                if part == "value:" and i + 1 < len(parts):
                    return parts[i + 1]
    return None


def validate_plan(
    domain_file: Path, problem_file: Path, plan_file: Path, val_path: str = "val"
) -> ValidationResult:
    """
    Validate a PDDL plan using VAL.

    Args:
        domain_file: Path to domain.pddl
        problem_file: Path to problem.pddl
        plan_file: Path to plan file
        val_path: Path to VAL validator executable

    Returns:
        ValidationResult with validity status and plan quality
    """
    cmd = [val_path, str(domain_file), str(problem_file), str(plan_file)]

    result = subprocess.run(cmd, capture_output=True, text=True)

    is_valid = "Plan valid" in result.stdout and result.returncode == 0
    plan_quality = extract_plan_quality(result.stdout) if is_valid else None

    return ValidationResult(
        is_valid=is_valid,
        plan_quality=plan_quality,
        stdout=result.stdout,
        stderr=result.stderr,
        returncode=result.returncode,
    )
