#!/usr/bin/env bash
# Fetch TinyCC and build a host libtcc so `cargo test` in native/ can Run VBR.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/third_party/tinycc"
HOST="$ROOT/third_party/tcc-host"

mkdir -p "$ROOT/third_party"
if [[ ! -f "$SRC/libtcc.c" ]]; then
    git clone --depth 1 https://github.com/TinyCC/tinycc.git "$SRC"
fi

# Host install — also produces tccdefs_.h, which the Android NDK build needs.
(
    cd "$SRC"
    ./configure --prefix="$HOST"
    make -j"$(nproc)"
    make install
)

echo "TinyCC host install: $HOST"
echo "Next:  cd $ROOT/native && cargo test"
