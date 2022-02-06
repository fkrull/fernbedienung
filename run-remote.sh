#!/bin/sh -eu
TARGET=armv7-unknown-linux-gnueabihf
BIN=target/$TARGET/debug/fernbedienung
cargo build --target=$TARGET --features baked-config
llvm-strip $BIN
scp $BIN $REMOTE:.cache/fernbedienung
exec ssh -t $REMOTE .cache/fernbedienung
