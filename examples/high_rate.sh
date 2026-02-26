#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 \
    --ports "80,443,22,3389,8080-8090" \
    --rate 5000
