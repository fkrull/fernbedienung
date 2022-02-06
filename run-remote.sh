#!/bin/sh -eu
TARGET=armv7-unknown-linux-musleabihf
BIN=target/$TARGET/debug/inputactiond
export RUSTFLAGS="-Clinker=rust-lld"
cargo build --target=$TARGET
llvm-strip $BIN
scp $BIN $REMOTE:.cache/inputactiond
exec ssh -t $REMOTE .cache/inputactiond
