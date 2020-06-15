#!/bin/bash

set -e

GG=./target/release/gg TIMEOUT=30s ./ci/gg-solve-all.sh ci/gg-problems-release.txt
GG=./target/debug/gg TIMEOUT=2m ./ci/gg-solve-all.sh ci/gg-problems-debug.txt