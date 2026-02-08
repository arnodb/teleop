#!/bin/sh

set -eu

cargo build --examples

export PID_FILE=$(mktemp)

cleanup() {
    rm "$PID_FILE"
}

trap cleanup EXIT

cargo run --example server &

sleep 2

PID=$(cat "$PID_FILE")

if [ -z "$PID" ]
then
    echo "Cannot find PID in $PID_FILE"
    exit 1
fi

for i in $(seq 1 10)
do
    cargo run --example client "$PID"
done

wait
