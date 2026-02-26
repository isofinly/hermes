#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 192.168.1.2 192.168.1.3 192.168.1.4 \
    --masscan-processes 4 \
    --parse-threads 4
