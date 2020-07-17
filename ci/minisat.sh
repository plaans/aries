#!/bin/bash

set -e

#cargo build --bin minisat
#EXE="./target/debug/minisat"
cargo build --release --bin minisat
EXE="./target/release/minisat"

export RUST_BACKTRACE=1

for filename in problems/cnf/sat/*.cnf; do
    echo "Running SAT: $filename"
    $EXE $filename --sat true >> /dev/null
done


for filename in problems/cnf/unsat/*.cnf; do
    echo "Running UNSAT: $filename"
    $EXE $filename --sat false >> /dev/null
done
