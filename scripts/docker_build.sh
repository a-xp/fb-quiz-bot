#!/usr/bin/env bash

echo "Building linux binary"
sudo chown -R rust:rust /home/rust/.cargo/git /home/rust/.cargo/registry /home/rust/src/target
cd backend || exit 1
cargo build --release --target x86_64-unknown-linux-gnu --bin server