#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 \
    --ports "80,443,8080"
