#!/usr/bin/env bash

ROOT="$(dirname "$0")/../"
ROOT="$(realpath $ROOT)"

RESULTS="$ROOT/test/.results/latest.txt"
mkdir -p $(dirname "$RESULTS")

bash "script/test_impl.sh" $@ | tee "$RESULTS"
EC=${PIPESTATUS[0]}

echo "INFO: Exiting with EC=$EC" | tee -a "$RESULTS"
exit $EC
