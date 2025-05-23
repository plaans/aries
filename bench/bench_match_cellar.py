from pathlib import Path
import re
import sqlite3
from subprocess import Popen, PIPE

from unified_planning.grpc.proto_writer import ProtobufWriter
from unified_planning.io.pddl_reader import PDDLReader
from unified_planning.shortcuts import Problem, IntType

db_file = Path(__file__).parent / "numeric_bench.db"
db = sqlite3.connect(db_file.as_posix())
cur = db.cursor()
cur.execute(
    """
    CREATE TABLE IF NOT EXISTS match_cellar (
        id INTEGER PRIMARY KEY,
        instance_id INTEGER,
        factorized BOOLEAN,
        use_bp BOOLEAN,
        num_matches INTEGER,
        num_fuses INTEGER,
        db_size INTEGER,
        cp_size INTEGER,
        num_bp INTEGER,
        computation_time REAL,
        quality REAL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"""
)


folder = Path("/home/roland/tyr/logs/aries/socs2025-match-cellar/")
for instance in folder.iterdir():
    if not instance.is_dir() or not instance.name.endswith("oneshot"):
        continue
    instance_id = int(instance.name.split("-")[0])

    reader = PDDLReader()
    pb: Problem = reader.parse_problem(
        (instance / "domain.pddl").as_posix(),
        (instance / "problem.pddl").as_posix(),
    )
    num_matches = int(str(pb.initial_value(pb.fluent("num_matches"))))
    pb.fluent("num_lit_matches")._typename = IntType(0, num_matches)
    output = Path("/tmp/problem.binpb")
    output.write_bytes(ProtobufWriter().convert(pb).SerializeToString())

    base_cmd = f"ARIES_UP_ASSUME_REALS_ARE_INTS=true cargo run --profile ci --bin up-server -- -l debug solve {output.as_posix()}"

    for factorized in [True, False]:
        for use_bp in [True, False]:
            cmd = f"ARIES_BORROW_PATTERN_CONSTRAINT={str(use_bp).lower()} ARIES_FACTORIZE_NUMERIC={str(factorized).lower()} {base_cmd}"
            print(
                f"Running {str(factorized).rjust(5)} {str(use_bp).rjust(5)} {instance_id:0>2}"
            )

            num_matches = 0
            num_fuses = 0
            db_size = -1
            cp_size = -1
            num_bp = -1
            computation_time = -1
            quality = 0

            with Popen(cmd, shell=True, stdout=PIPE) as proc:
                for line in proc.stdout:
                    line = line.decode("utf-8").strip()
                    if "DB size" in line:
                        db_size = int(line.split(":")[1].strip())
                    if "# constraints" in line:
                        cp_size = int(line.split(":")[1].strip())
                    if "num_borrow_patterns" in line:
                        num_bp = int(line.split("=")[1].replace("\x1b[0m", "").strip())
                    if match := re.match(r"^\[(.+)s\] Solved", line):
                        computation_time = float(match.group(1))
                    if match := re.match(r"^(.+)\s*:\s*\((.+)\)\s*\[(.+)\]", line):
                        quality = max(
                            quality, float(match.group(1)) + float(match.group(3))
                        )
                        if match.group(2) == "mend_fuse":
                            num_fuses += 1
                        elif match.group(2) == "light_match":
                            num_matches += 1
                        else:
                            raise ValueError(f"Unknown match type: {match.group(2)}")

            print(
                f"  {num_fuses} {num_matches} {db_size} {cp_size} {num_bp} {computation_time} {quality}"
            )
            cur.execute(
                """
                INSERT INTO match_cellar
                (instance_id, factorized, use_bp, num_fuses, num_matches, db_size, cp_size, num_bp, computation_time, quality)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
                (
                    instance_id,
                    factorized,
                    use_bp,
                    num_fuses,
                    num_matches,
                    db_size,
                    cp_size,
                    num_bp,
                    computation_time,
                    quality,
                ),
            )
            db.commit()

db.close()
