
set -e # Exit on first error

# Path TO GG (defaults to debug build)
PLANNER="${PLANNER:-target/debug/lcp}"
HDDL_VAL="${HDDL_VAL:-ext/pandaPIparser}"

# Time allowed for each run (defaults to 30s)
TIMEOUT="${TIMEOUT:-30s}"

echo "Building..."
cargo build

TEST_PROBLEMS="ci/hddl_test_problems.txt"

# Write all test commands to temporary file
COMMANDS=$(mktemp)
for PROB_FILE in $(cat $TEST_PROBLEMS | grep -v '^#')
do
    DOM_FILE="$(dirname $PROB_FILE)/domain.hddl"
    PLAN_FILE=$(mktemp)
    COMMAND="timeout ${TIMEOUT} ${PLANNER} ${PROB_FILE} -o ${PLAN_FILE} &&  ${HDDL_VAL} -l -verify ${DOM_FILE} ${PROB_FILE} ${PLAN_FILE}"

    echo $COMMAND >> $COMMANDS
done

# run all commands in parallel
cat $COMMANDS | parallel -v --halt-on-error now,fail=1 '{}'




