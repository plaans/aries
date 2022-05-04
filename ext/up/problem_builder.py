import os

try:
    from unified_planning.grpc.proto_writer import ProtobufWriter
    from unified_planning.test.examples import get_example_problems
except ImportError:
    raise ImportError(
        "Unified Planning is not found in the current workspace. Please install it."
    )


class BuildProblems:
    OUT_DIR = os.path.join(os.path.dirname(__file__), "bins")
    WRITER = ProtobufWriter()
    examples = list(get_example_problems().keys())

    def __init__(self) -> None:
        if not os.path.exists(self.OUT_DIR):
            os.mkdir(self.OUT_DIR)
        try:
            os.mkdir(os.path.join(self.OUT_DIR, "problems"))
            os.mkdir(os.path.join(self.OUT_DIR, "plans"))
        except FileExistsError:
            pass

    def export_problems(self):
        for p in self.examples:
            with open(os.path.join(self.OUT_DIR, "problems/" + p + ".bin"), "wb") as f:
                print(f"Exporting problem {p}")
                data = get_example_problems()[p].problem
                try:
                    data = self.WRITER.convert(data)
                except Exception as e:
                    continue
                f.write(data.SerializeToString())

    def export_plans(self):
        for p in self.examples:
            with open(os.path.join(self.OUT_DIR, "plans/" + p + ".bin"), "wb") as f:
                print(f"Exporting plan {p}")
                data = get_example_problems()[p].plan
                try:
                    data = self.WRITER.convert(data)
                except Exception as e:
                    continue
                f.write(data.SerializeToString())


def main():
    builder = BuildProblems()
    builder.export_problems()
    builder.export_plans()


if __name__ == "__main__":
    main()
