#!/bin/bash
set -e

sudo cargo run --release -- \
    --target 192.168.1.1 \
    --smtp-server smtp.example.com \
    --smtp-port 587 \
    --smtp-username "scan@example.com" \
    --smtp-password "smtp_password" \
    --email-from "hermes@example.com" \
    --email-to "admin@example.com" \
    --email-subject "Scan Results for 192.168.1.1"
