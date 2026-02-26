#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 \
    --database scan_results.sqlite3
