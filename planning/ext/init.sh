
set -e

# CD into the directory with this file
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
cd $DIR

git clone https://github.com/plaans/tyr-ipc-domains.git pddl
cd pddl && git checkout be7e1a16928c28dd99f4e69d3f1a10c876cb838c

cd $DIR
git clone https://github.com/panda-planner-dev/ipc2020-domains.git hddl
cd hddl && git checkout 9e313248244a0a13302ae262f42ef446f43e4182
