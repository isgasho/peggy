#!/bin/bash
set -eu

FILE=/contracts
if test -f "$FILE"; then
echo "Contracts already deployed, running tests"
else 
echo "Testnet is not started yet, please wait before running tests"
exit 0
fi 

set +e
killall -9 test-runner
set -e

pushd /peggy/orchestrator/test_runner
RUST_BACKTRACE=full NO_GAS_OPT=1 RUST_LOG=INFO PATH=$PATH:$HOME/.cargo/bin cargo run --release --bin test-runner
