#!/bin/bash

set -e

cargo test

cargo build --release
EXE="./target/release/minisat"

for filename in problems/cnf/sat/*.cnf; do
    echo "Running SAT: $filename"
    $EXE $filename --sat true >> /dev/null
done


for filename in problems/cnf/unsat/*.cnf; do
    echo "Running UNSAT: $filename"
    $EXE $filename --sat false >> /dev/null
done
