# Runs APE's validator on a all plan files in planning/problem/upf
#
#
# Some pointer for generating plans: (here for tamer)
#     cd planning/problems/upf
#     find . -type d -exec sh -c 'cd "{}" && up oneshot-planning --pddl domain.pddl problem.pddl --engine tamer --plan problem.tamer.plan || true' \;

set -e # Exit on first error

# Path to planner and validators (defaults to ci build)
APE="${PLANNER:-target/ci/ape} "
HDDL_VAL="${HDDL_VAL:-planning/ext/val-hddl}"
PDDL_VAL="${PDDL_VAL:-planning/ext/val-pddl}"

# Time allowed for each run (defaults to 90s)
TIMEOUT="${TIMEOUT:-5s}"

echo "Building..."
cargo build --profile ci --bin ape

PLAN_FILES=$(find planning/problems/upf -name *.plan | sort)

for PLAN_FILE in $PLAN_FILES
do
    echo ""
    echo "Plan: ${PLAN_FILE}"
    PROBLEM_FILE=`${APE} find-problem ${PLAN_FILE}`
    DOMAIN_FILE=`${APE} find-domain ${PROBLEM_FILE}`

    VAL_CMD="${PDDL_VAL} ${DOMAIN_FILE} ${PROBLEM_FILE} ${PLAN_FILE}"

    # Run VAL with some post-processing to
    #   - exit with error on invalid plan
    #   - extract only the computed plan quality on the stdout and save in in $PLAN_QUALITY
    PLAN_QUALITY=`${VAL_CMD} | awk -v cmd="${VAL_CMD}" '/Plan valid/{ok=1} ok && /Final value:/ {print $3; exit} END{if(!ok) {print "INVALID (from val)\n> " cmd >"/dev/stderr"; exit 1}}'`
    echo "VAL plan quality: ${PLAN_QUALITY}"
    timeout ${TIMEOUT} ${APE} validate ${PLAN_FILE} --expected-objective ${PLAN_QUALITY}

    # Just check that the plan-optimizer does not crash...
    # echo "OPT PRESENCE:  " optimize-plan ${PLAN_FILE} -r action-presence
    # timeout ${TIMEOUT} ${APE} optimize-plan ${PLAN_FILE} -r action-presence > /dev/null
    # echo "OPT TIME:  " optimize-plan ${PLAN_FILE} -r start-time
    # timeout ${TIMEOUT} ${APE} optimize-plan ${PLAN_FILE} -r start-time > /dev/null
    # echo "OPT PRESENCE TIME:  " optimize-plan ${PLAN_FILE} -r action-presence -r start-time
    timeout ${TIMEOUT} ${APE} optimize-plan ${PLAN_FILE} -r action-presence -r start-time > /dev/null
done
