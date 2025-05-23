from pathlib import Path
import re
import sqlite3
from subprocess import Popen, PIPE

db_file = Path(__file__).parent / "numeric_bench.db"
db = sqlite3.connect(db_file.as_posix())
cur = db.cursor()
cur.execute(
    """
    CREATE TABLE IF NOT EXISTS rcpsp (
        id INTEGER PRIMARY KEY,
        instance_id INTEGER,
        factorized BOOLEAN,
        use_bp BOOLEAN,
        num_jobs INTEGER,
        db_size INTEGER,
        cp_size INTEGER,
        num_bp INTEGER,
        computation_time REAL,
        quality REAL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"""
)


folder = Path("/home/roland/tyr/logs/aries/socs2025-rcpsp/")
for instance in folder.iterdir():
    if not instance.is_dir() or not instance.name.endswith("oneshot"):
        continue
    instance_id = int(instance.name.split("-")[0])
    pb = instance / "problem.binpb"
    base_cmd = f"cargo run --profile ci --bin up-server -- -l debug solve {pb.as_posix()}"
    
    for factorized in [True, False]:
        for use_bp in [True, False]:
            cmd = f"ARIES_BORROW_PATTERN_CONSTRAINT={str(use_bp).lower()} ARIES_FACTORIZE_NUMERIC={str(factorized).lower()} {base_cmd}"
            print(f"Running {str(factorized).rjust(5)} {str(use_bp).rjust(5)} {instance_id:0>2}")

            num_jobs = -1
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
                    if match := re.match(r"^(.+)\s*:\s*\(a(\d+)\)\s*\[(.+)\]", line):
                        num_jobs = max(num_jobs, int(match.group(2)) + 1)
                        quality = max(quality, float(match.group(1)) + float(match.group(3)))

            print(f"  {num_jobs} {db_size} {cp_size} {num_bp} {computation_time} {quality}")
            cur.execute(
                """
                INSERT INTO rcpsp
                (instance_id, factorized, use_bp, num_jobs, db_size, cp_size, num_bp, computation_time, quality)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
                (instance_id, factorized, use_bp, num_jobs, db_size, cp_size, num_bp, computation_time, quality),
            )
            db.commit()

db.close()
