#!/usr/bin/python3

# Script that run the UP integration tests.

import report  # from planning-test-case, should probably change once the upstream repository stabilizes
from unified_planning.shortcuts import *

# declare aries val
get_environment().factory.add_engine("aries-val", "up_aries", "AriesVal")


mode = sys.argv[1]
pb = sys.argv[2] if sys.argv[2:] else ""

if mode.lower() == "val":
    errors = report.report_validation("aries-val", problem_prefix=pb)
elif mode.lower() == "solve":
    errors = report.report_oneshot("aries", problem_prefix=pb)
else:
    raise ValueError(f"Unknown mode: {mode}")


assert len(errors) == 0
