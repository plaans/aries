#!/usr/bin/env bash
cd "$(dirname "$0")"
mkdir problems
git clone https://github.com/Cyril-Grelier/gc_instances.git
mv gc_instances/original_graphs/* problems/
rm -rf gc_instances
