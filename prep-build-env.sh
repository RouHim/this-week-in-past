#!/usr/bin/env bash

declare -A BUILD_TARGET
BUILD_TARGET[amd64]="x86_64-unknown-linux-musl"
BUILD_TARGET[arm64]="aarch64-unknown-linux-musl"
BUILD_TARGET[arm]="armv7-unknown-linux-musleabihf"

declare -A BUILDER_TAG
BUILDER_TAG[amd64]="x86_64-musl"
BUILDER_TAG[arm64]="rust-musl-cross:aarch64-musl"
BUILDER_TAG[arm]="rust-musl-cross:armv7-musleabihf"