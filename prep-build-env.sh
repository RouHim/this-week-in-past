#!/usr/bin/env bash
#
# Description:
#   Provides a function to build a static binary for the specified cpu arch.
#   This script utilizes this project: https://github.com/rust-cross/rust-musl-cross
#
# Parameter:
#   $1 - CPU arch to build. Check all available docker image tags here: https://github.com/rust-cross/rust-musl-cross
#
# How to use:
#   Just source this file and then run: build-rust-static-bin <desired-cpu-arch>
#
# Example:
#   build-rust-static-bin aarch64-musl
#
# # # #

function build-rust-static-bin() {
    echo "Building arch: $1"
    docker run --rm -e CARGO_NET_GIT_FETCH_WITH_CLI=true -v "$(pwd)":/home/rust/src messense/rust-musl-cross:"${1}" cargo build --release
}