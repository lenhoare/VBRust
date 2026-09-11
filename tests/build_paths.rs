//! `$VBR_BUILD` relocates the generated cargo project; `$VBR_TARGET` (and the
//! default cache) keep `target/` out of the source tree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn vbr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vbr"))
}

fn scratch(tag: &str) -> PathBuf {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("vbr-{tag}-{}-{n}", std::process::id()));
    fs::create_dir_all(&p).unwrap();
    p
}

fn find_cargo_toml(root: &Path) -> PathBuf {
    for e in fs::read_dir(root).unwrap().flatten() {
        let p = e.path();
        if p.join("Cargo.toml").is_file() {
            return p.join("Cargo.toml");
        }
    }
    panic!("no generated Cargo.toml under {}", root.display());
}

#[test]
fn vbr_build_env_relocates_project_and_points_target_at_cache() {
    let src = scratch("src");
    let out = scratch("out");
    let artifacts = scratch("target");
    fs::write(
        src.join("main.vbr"),
        "Function Main()\n    Debug.Print \"ok\"\nEnd Function\n",
    )
    .unwrap();

    let result = vbr()
        .arg("build")
        .arg(&src)
        .env("VBR_BUILD", &out)
        .env("VBR_TARGET", &artifacts)
        .output()
        .expect("run vbr build");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success(),
        "vbr build failed: {stderr}"
    );
    assert!(
        !src.join("build").exists(),
        "in-tree build/ should not appear when VBR_BUILD is set"
    );

    let cargo = find_cargo_toml(&out);
    let build_dir = cargo.parent().unwrap();
    assert!(build_dir.join("src/main.rs").is_file());
    let cfg = fs::read_to_string(build_dir.join(".cargo/config.toml")).unwrap();
    let want = artifacts.display().to_string().replace('\\', "/");
    assert!(
        cfg.contains(&want),
        "config.toml should point at VBR_TARGET ({want}):\n{cfg}"
    );
    assert!(
        stderr.contains(&build_dir.display().to_string()),
        "vbr should print the relocated project path: {stderr}"
    );

    let _ = fs::remove_dir_all(&src);
    let _ = fs::remove_dir_all(&out);
    let _ = fs::remove_dir_all(&artifacts);
}

#[test]
fn default_build_stays_beside_sources() {
    let src = scratch("beside");
    fs::write(
        src.join("main.vbr"),
        "Function Main()\n    Debug.Print \"ok\"\nEnd Function\n",
    )
    .unwrap();

    let result = vbr()
        .arg("build")
        .arg(&src)
        .env_remove("VBR_BUILD")
        .output()
        .expect("run vbr build");
    assert!(
        result.status.success(),
        "vbr build failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(src.join("build/Cargo.toml").is_file());
    let cfg = fs::read_to_string(src.join("build/.cargo/config.toml")).unwrap();
    assert!(
        cfg.contains("target-dir"),
        "in-tree build still needs a shared cargo target-dir:\n{cfg}"
    );

    let _ = fs::remove_dir_all(&src);
}
