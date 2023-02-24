import os
from pathlib import Path
import sys

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


class BuildProblems:
    OUT_DIR = Path(__file__).resolve().parent / "bins"
    WRITER = ProtobufWriter()
    examples = list(get_example_problems().keys())

    def __init__(self) -> None:
        self.OUT_DIR.mkdir(parents=True, exist_ok=True)
        (self.OUT_DIR / "problems").mkdir(parents=True, exist_ok=True)
        (self.OUT_DIR / "plans").mkdir(parents=True, exist_ok=True)

    def export_problems(self):
        for p in self.examples:
            with open(os.path.join(self.OUT_DIR, "problems/" + p + ".bin"), "wb") as f:
                print(f"Exporting problem {p}")
                data = get_example_problems()[p].problem
                try:
                    data = self.WRITER.convert(data)
                except Exception as _e:
                    continue
                f.write(data.SerializeToString())

    def export_plans(self):
        for p in self.examples:
            with open(os.path.join(self.OUT_DIR, "plans/" + p + ".bin"), "wb") as f:
                print(f"Exporting plan {p}")
                data = get_example_problems()[p].plan
                try:
                    data = self.WRITER.convert(data)
                except Exception as _e:
                    continue
                f.write(data.SerializeToString())


def main():
    builder = BuildProblems()
    builder.export_problems()
    builder.export_plans()


if __name__ == "__main__":
    main()
