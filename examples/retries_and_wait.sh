#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 \
    --max-retries 3 \
    --wait 10
