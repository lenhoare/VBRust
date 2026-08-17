# VBR for Android

A Turbo Pascal–inspired IDE (same chrome as desktop **TIDE**) that **runs** VBR
on a phone. Spec: [`android_spec.md`](../android_spec.md). The generated-code
pane is **C** (this phone has no `rustc`), with a VBR↔C line map so it scrolls
with the cursor.

The Rust toolchain does **not** live on the phone. The VBR compiler is a
prebuilt `.so`. `Debug.Print` / `Function Main()` and `Screen` both run in that
`.so` (an AST interpreter). TinyCC is linked for host tests; Android will not
let it JIT (`tcc_relocate` hangs).

## Host tests (no phone, no NDK)

These prove the pipeline this machine can run today: VBR → C → TinyCC → stdout.

```sh
./scripts/fetch-tcc.sh          # once: clone TinyCC, build host libtcc
cd native && cargo test
```

`run_hello_via_tcc` and `run_maths_example` fail the build if Run doesn't print
what `vbr c` / `tcc -run` would.

## Building the APK

You need:

1. Android Studio (or the SDK at `~/Android/Sdk`) **plus an NDK**.
2. Rust (`rustup`), and once: `cargo install cargo-ndk`.
3. TinyCC sources: `./scripts/fetch-tcc.sh` (same script as the host tests).

Then:

```sh
cp local.properties.example local.properties   # fix sdk.dir if needed
./scripts/build-native.sh                      # libvbr_android.so → jniLibs/
# Open this folder in Android Studio and Run, or:
# ./gradlew :app:installDebug
```

`scripts/build-native.sh` cross-compiles the Rust crate (compiler + libtcc +
JNI) for `arm64-v8a` and `x86_64` (emulator).

If the native library isn't in the APK, the app still opens and the editor
works; Run tells you to rebuild with the NDK.

## What Run covers

**`Function Main()`** (`Debug.Print`, maths, core language) runs in the native
interpreter. **`Screen` programs open the TUI host** (same widgets as desktop
`tide`, clickable). `Window`/`Page`, Http/Json/Database, and POSIX stdlib
namespaces (`FileSystem`, …) are refused with a teaching message rather than a
linker dump.

## Layout

```
vbr-android/
  native/     Rust cdylib (vbr::compile_c + TinyCC + JNI)
  runner/     tcc_run.c — in-process compile-and-run
  app/        Kotlin WebView TIDE shell + bundled editor
  scripts/    fetch-tcc.sh, build-native.sh
  third_party/  TinyCC (fetched, not committed)
```
