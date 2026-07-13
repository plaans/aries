#!/usr/bin/env python3

import argparse
import sys
from pathlib import Path
import json

def main():
    parser = argparse.ArgumentParser()

    parser.add_argument(
        "files",
        nargs="+",
        metavar="FILE",
    )

    parser.add_argument(
        "--out",
        metavar="FILE",
        required=True,
    )

    args = parser.parse_args()

    in_files = [Path(f) for f in args.files]
    out_file = Path(args.out)
    print(out_file)

    aggr_json_numdecisions = {}

    for in_file in in_files:
        with in_file.open("r", encoding="utf-8") as f:
            d = json.load(f)

            for entry in d['entries']:
                if entry['status'] == "SolvedUnsat":
                    aggr_json_numdecisions.setdefault(entry['problem']['name'], {})[d['run_name']] = entry['metrics']['NumDecisions']
    
    with out_file.open("w", encoding="utf-8") as f:
        json.dump({ "NumDecisions": aggr_json_numdecisions }, f, indent=2, sort_keys=True)


if __name__ == "__main__":
    sys.exit(main())
