#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 \
    --masscan-arg --exclude \
    --masscan-arg 192.168.1.254 \
    --masscan-arg --adapter \
    --masscan-arg eth0
