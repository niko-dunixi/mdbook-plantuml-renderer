#!/usr/bin/env bash
set -ex
cargo fmt
cargo install --path .
