//! Build script: compile `tcc_run.c` and link libtcc when TinyCC is present.
//!
//! Host: `scripts/fetch-tcc.sh` installs libtcc into `third_party/tcc-host`.
//! Android: we compile TinyCC's sources for the target ABI (needs `tccdefs_.h`
//! from a host configure, which the fetch script also produces).

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap(); // vbr-android/
    let tcc_src = root.join("third_party/tinycc");
    let tcc_host = root.join("third_party/tcc-host");
    let runner = root.join("runner/tcc_run.c");
    let target = env::var("TARGET").unwrap_or_default();
    let android = target.contains("android");

    println!("cargo:rerun-if-changed={}", runner.display());
    println!("cargo:rerun-if-changed={}", root.join("runner/tcc_run.h").display());
    println!("cargo:rerun-if-env-changed=VBR_TCCDIR");

    if android {
        if !tcc_src.join("libtcc.c").is_file() {
            println!("cargo:warning=TinyCC sources missing — run vbr-android/scripts/fetch-tcc.sh");
            return;
        }
        compile_tinycc(&tcc_src, &target);
        compile_runner(&runner, &tcc_src, &tcc_src); // libtcc.h lives in the source tree
        println!("cargo:rustc-cfg=has_tcc");
        let tccdir = env::var("VBR_TCCDIR").unwrap_or_else(|_| ".".into());
        println!("cargo:rustc-env=VBR_TCCDIR={tccdir}");
        return;
    }

    let lib = tcc_host.join("lib/libtcc.a");
    if !lib.is_file() {
        println!(
            "cargo:warning=TinyCC host install not found at {} — run scripts/fetch-tcc.sh. Tests will skip in-process Run.",
            lib.display()
        );
        return;
    }

    compile_runner(&runner, &tcc_host.join("include"), &tcc_src);
    println!("cargo:rustc-link-search=native={}", tcc_host.join("lib").display());
    println!("cargo:rustc-link-lib=static=tcc");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=m");
    println!("cargo:rustc-cfg=has_tcc");
    let tccdir = env::var("VBR_TCCDIR").unwrap_or_else(|_| {
        tcc_host.join("lib/tcc").to_string_lossy().into_owned()
    });
    println!("cargo:rustc-env=VBR_TCCDIR={tccdir}");
}

fn compile_runner(runner: &Path, include: &Path, tcc_src: &Path) {
    let mut b = cc::Build::new();
    b.file(runner)
        .include(include)
        .include(tcc_src)
        .include(runner.parent().unwrap())
        .warnings(false);
    b.compile("vbr_tcc_run");
}

fn compile_tinycc(src: &Path, target: &str) {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let (def, extra) = if target.starts_with("aarch64") {
        ("TCC_TARGET_ARM64", &["arm64-gen.c", "arm64-link.c", "arm64-asm.c"][..])
    } else if target.contains("arm") && !target.contains("aarch64") {
        ("TCC_TARGET_ARM", &["arm-gen.c", "arm-link.c", "arm-asm.c"][..])
    } else {
        ("TCC_TARGET_X86_64", &["x86_64-gen.c", "x86_64-link.c", "i386-asm.c"][..])
    };

    let config = format!(
        "/* generated for {target} */\n\
         #define TCC_VERSION \"0.9.28rc\"\n\
         #define {def} 1\n\
         #define CONFIG_TCC_PREDEFS 1\n\
         #ifndef CONFIG_TCCDIR\n\
         #define CONFIG_TCCDIR \".\"\n\
         #endif\n"
    );
    std::fs::write(out.join("config.h"), config).unwrap();

    let mut files = vec![
        "libtcc.c",
        "tccpp.c",
        "tccgen.c",
        "tccdbg.c",
        "tccelf.c",
        "tccasm.c",
        "tccrun.c",
    ];
    files.extend_from_slice(extra);

    let mut b = cc::Build::new();
    b.include(&out)
        .include(src)
        .define("ONE_SOURCE", "0")
        .define(def, None)
        .warnings(false)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-sign-compare");
    for f in files {
        b.file(src.join(f));
    }
    b.compile("tcc");
    println!("cargo:rustc-link-lib=static=tcc");
    println!("cargo:rustc-link-lib=dylib=m");
    println!("cargo:rustc-link-lib=dylib=dl");
}
