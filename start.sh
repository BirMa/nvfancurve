#! /usr/bin/env sh

export RUSTFLAGS="-C target-cpu=native"
export RUST_BACKTRACE=1
#export RUST_LOG=info
#export RUST_LOG=debug
exec cargo run
