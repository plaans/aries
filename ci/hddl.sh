
set -e # Exit on first error

# Path TO GG (defaults to debug build)
PARSER="${PARSER:-target/debug/pddl}"

# Time allowed for each run (defaults to 10s)
TIMEOUT="${TIMEOUT:-10s}"

echo "Building..."
cargo build
#cargo build --release

input="ci/hddl_test_problems.txt"


# Read problems from first argument or standard input if not provided
#[ $# -ge 1 -a -f "$1" ] && input="$1" || input="-"
cat $input | grep -v '^#' |  parallel -v --halt-on-error now,fail=1 "timeout ${TIMEOUT} ${PARSER} {}"
