#!/bin/sh

rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

docker build -t rouhim/this-week-in-past:latest .

echo "run with: docker-compose up"