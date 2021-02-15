
set -e # Exit on first error

# Path TO GG (defaults to debug build)
PLANNER="${PLANNER:-target/release/lcp}"
HDDL_VAL="${HDDL_VAL:-ext/pandaPIparser}"
PDDL_VAL="${PDDL_VAL:-ext/validate}"

# Time allowed for each run (defaults to 30s)
TIMEOUT="${TIMEOUT:-60s}"

echo "Building..."
cargo build --release --bin lcp

# Write all test commands to temporary file
COMMANDS=$(mktemp)

# Add HDDL problems

HDDL_PROBLEMS=$(find problems/hddl -name instance-1.hddl)

for PROB_FILE in $HDDL_PROBLEMS
do
    DOM_FILE="$(dirname $PROB_FILE)/domain.hddl"
    PLAN_FILE=$(mktemp)
    COMMAND="timeout ${TIMEOUT} ${PLANNER} ${PROB_FILE} -o ${PLAN_FILE} &&  ${HDDL_VAL} -l -verify ${DOM_FILE} ${PROB_FILE} ${PLAN_FILE}"

    echo "$COMMAND" >> "$COMMANDS"
done

# Add pddl problems

PDDL_PROBLEMS=$(find problems/pddl -name instance-1.pddl)

for PROB_FILE in $PDDL_PROBLEMS
do
    DOM_FILE="$(dirname $PROB_FILE)/domain.pddl"
    PLAN_FILE=$(mktemp)
    COMMAND="timeout ${TIMEOUT} ${PLANNER} ${PROB_FILE} -o ${PLAN_FILE} &&  ${PDDL_VAL} ${DOM_FILE} ${PROB_FILE} ${PLAN_FILE}"

    echo $COMMAND >> $COMMANDS
done

# run all commands in parallel
cat $COMMANDS | parallel -v --halt-on-error now,fail=1 '{}'

echo "======== Successful runs ======="
cat "$COMMANDS"


