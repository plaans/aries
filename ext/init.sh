
set -e

# CD into the directory with this file
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
cd $DIR

git clone --depth 1 https://github.com/potassco/pddl-instances.git pddl

git clone --depth 1 https://github.com/panda-planner-dev/ipc2020-domains.git hddl
