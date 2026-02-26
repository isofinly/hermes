#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 192.168.1.2 192.168.1.3
