#!/usr/bin/env bash
# Cross-compile libvbr_android.so into app/src/main/jniLibs/<abi>/
# Needs: rustup target, cargo-ndk, Android NDK.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
# Pick the newest NDK if ANDROID_NDK_HOME isn't set.
if [[ -z "${ANDROID_NDK_HOME:-}" && -d "$ANDROID_HOME/ndk" ]]; then
    export ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME/ndk"/* 2>/dev/null | sort | tail -1)"
fi
if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "$ANDROID_NDK_HOME" ]]; then
    echo "Set ANDROID_NDK_HOME (or install an NDK under \$ANDROID_HOME/ndk)." >&2
    exit 1
fi

if ! command -v cargo-ndk >/dev/null 2>&1; then
    echo "Installing cargo-ndk…"
    cargo install cargo-ndk --locked
fi

for t in aarch64-linux-android x86_64-linux-android; do
    rustup target add "$t" >/dev/null
done

cd "$ROOT/native"
cargo ndk -t arm64-v8a -t x86_64 \
    -o "$ROOT/app/src/main/jniLibs" \
    build --release --features jni-bridge

echo "Native libs → $ROOT/app/src/main/jniLibs"
