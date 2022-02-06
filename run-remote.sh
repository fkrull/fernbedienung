#!/bin/sh -eu
TARGET=armv7-unknown-linux-gnueabihf
BIN=target/$TARGET/debug/inputactiond
cargo build --target=$TARGET
llvm-strip $BIN
scp $BIN $REMOTE:.cache/inputactiond
exec ssh -t $REMOTE .cache/inputactiond
