
set -e # Exit on first error

# Path to planner and validators (defaults to release build)
PLANNER="${PLANNER:-target/debug/lcp}"
HDDL_VAL="${HDDL_VAL:-ext/val-hddl}"
PDDL_VAL="${PDDL_VAL:-ext/val-pddl}"

# Time allowed for each run (defaults to 90s)
TIMEOUT="${TIMEOUT:-90s}"

echo "Building..."
cargo build --bin lcp

# Write all test commands to temporary file
COMMANDS=$(mktemp)

# Add HDDL problems

HDDL_PROBLEMS=$(find problems/hddl -name instance-1.hddl)

for PROB_FILE in $HDDL_PROBLEMS
do
    DOM_FILE="$(dirname "$PROB_FILE")/domain.hddl"
    PLAN_FILE=$(mktemp)
    COMMAND="timeout ${TIMEOUT} ${PLANNER} ${PROB_FILE} -o ${PLAN_FILE} &&  ${HDDL_VAL} -l -verify ${DOM_FILE} ${PROB_FILE} ${PLAN_FILE}"

    echo "$COMMAND" >> "$COMMANDS"
done

# Add pddl problems

PDDL_PROBLEMS=$(find problems/pddl -name instance-1.pddl)

for PROB_FILE in $PDDL_PROBLEMS
do
    DOM_FILE="$(dirname "$PROB_FILE")/domain.pddl"
    PLAN_FILE=$(mktemp)
    COMMAND="timeout ${TIMEOUT} ${PLANNER} ${PROB_FILE} -o ${PLAN_FILE} &&  ${PDDL_VAL} ${DOM_FILE} ${PROB_FILE} ${PLAN_FILE}"

    echo "$COMMAND" >> "$COMMANDS"
done


# limit (global?) memory usage to 1GB
ulimit -m 1000000
ulimit -v 1000000

# run all commands in parallel
cat "$COMMANDS" | parallel -v --halt-on-error now,fail=1 '{}'

# if we reach this point, it means that no error occurred while running planners
echo "======== Successful runs ======="
cat "$COMMANDS"


