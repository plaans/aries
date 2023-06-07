
set -e # Exit on first error

# Path to planner and validators (defaults to release build)
PLANNER="${PLANNER:-target/ci/lcp}"
FIND_DOMAIN="target/ci/planning-domain"
HDDL_VAL="${HDDL_VAL:-planning/ext/val-hddl}"
PDDL_VAL="${PDDL_VAL:-planning/ext/val-pddl}"

# Time allowed for each run (defaults to 90s)
TIMEOUT="${TIMEOUT:-90s}"

echo "Building..."
cargo build --profile ci --bin lcp
cargo build --profile ci --bin planning-domain

# Write all test commands to temporary file
COMMANDS=$(mktemp)

# Add HDDL problems

HDDL_PROBLEMS=$(find planning/problems/ -name *.pb.hddl)

for PROB_FILE in $HDDL_PROBLEMS
do
    DOM_FILE="$(${FIND_DOMAIN} ${PROB_FILE})"
    PLAN_FILE=$(mktemp)
    COMMAND="timeout ${TIMEOUT} ${PLANNER} ${PROB_FILE} -o ${PLAN_FILE} &&  ${HDDL_VAL} -l -verify ${DOM_FILE} ${PROB_FILE} ${PLAN_FILE}"

    echo "$COMMAND" >> "$COMMANDS"
done

# Add pddl problems

PDDL_PROBLEMS=$(find planning/problems/ -name *.pb.pddl)

for PROB_FILE in $PDDL_PROBLEMS
do
    DOM_FILE="$(${FIND_DOMAIN} ${PROB_FILE})"
    PLAN_FILE=$(mktemp)
    COMMAND="timeout ${TIMEOUT} ${PLANNER} ${PROB_FILE} -o ${PLAN_FILE} &&  ${PDDL_VAL} ${DOM_FILE} ${PROB_FILE} ${PLAN_FILE}"

    echo "$COMMAND" >> "$COMMANDS"
done


# limit (global?) memory usage to 1GB
ulimit -m 1000000
ulimit -v 1000000

# run all commands in parallel
cat "$COMMANDS" | parallel --max-procs 33% --use-cores-instead-of-threads -v --halt-on-error now,fail=1 '{}'

# if we reach this point, it means that no error occurred while running planners
echo "======== Successful runs ======="
cat "$COMMANDS"


