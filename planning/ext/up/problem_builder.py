"""Module to export the problems and the plans of the unified_planning module in binary."""

from pathlib import Path
import sys
from typing import IO

# Use the local version of the UP in the `planning/ext/up/unified_planning` git submodule
upf_path = Path(__file__).resolve().parent / "unified_planning"
sys.path.append(upf_path.as_posix())

try:
    from unified_planning.grpc.proto_writer import ProtobufWriter
    from unified_planning.test.examples import get_example_problems
except ImportError as err:
    raise ImportError(
        "Unified Planning is not found in the current workspace. Please install it."
    ) from err


class Factory:
    """Factory for the problems and the plans of the unified_planning module in binary."""

    _OUT_DIR = Path(__file__).resolve().parent / "bins"
    _writer = ProtobufWriter()
    _examples = get_example_problems()

    def __init__(self) -> None:
        self._OUT_DIR.mkdir(parents=True, exist_ok=True)
        (self._OUT_DIR / "problems").mkdir(parents=True, exist_ok=True)
        (self._OUT_DIR / "plans").mkdir(parents=True, exist_ok=True)

    @classmethod
    def problem_file(cls, name: str) -> Path:
        """Returns the path of the problem binary with the given name."""
        return cls._OUT_DIR / "problems" / f"{name}.bin"

    @classmethod
    def plan_file(cls, name: str) -> Path:
        """Returns the path of the plan binary with the given name."""
        return cls._OUT_DIR / "plans" / f"{name}.bin"

    @classmethod
    def _export(cls, data, file: IO):
        """Exports the data in the file in binary."""
        try:
            data = cls._writer.convert(data)
        except Exception:  # nosec: B112  # pylint: disable = W0718
            return
        file.write(data.SerializeToString())

    @classmethod
    def export_problems(cls):
        """Exports all the problems."""
        for name, (problem, _) in cls._examples.items():
            with open(cls.problem_file(name), "wb") as file:
                print(f"Exporting problem {name}")
                cls._export(problem, file)

    @classmethod
    def export_plans(cls):
        """Exports all the plans."""
        for name, (_, plan) in cls._examples.items():
            with open(cls.plan_file(name), "wb") as file:
                print(f"Exporting plan {name}")
                cls._export(plan, file)


if __name__ == "__main__":
    Factory.export_problems()
    Factory.export_plans()
