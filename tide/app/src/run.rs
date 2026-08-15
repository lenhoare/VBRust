//! Suspend the TUI, run `vbr`, restore — Turbo Pascal compile-and-run loop.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;

type TideTerminal = Terminal<CrosstermBackend<Stdout>>;

/// `VBR_BIN` if set, else a `vbr` built next to Tide / in a parent `target/`,
/// else the name `vbr` for PATH lookup.
pub fn vbr_bin() -> String {
    if let Ok(p) = env::var("VBR_BIN") {
        if !p.trim().is_empty() {
            return p;
        }
    }
    find_vbr_bin()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vbr".into())
}

fn vbr_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["vbr.exe", "vbr"]
    } else {
        &["vbr", "vbr.exe"]
    }
}

fn vbr_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in vbr_names() {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn vbr_in_target(root: &Path) -> Option<PathBuf> {
    for profile in ["debug", "release"] {
        if let Some(p) = vbr_in_dir(&root.join("target").join(profile)) {
            return Some(p);
        }
    }
    None
}

fn find_vbr_bin() -> Option<PathBuf> {
    if let Ok(td) = env::var("CARGO_TARGET_DIR") {
        for profile in ["debug", "release"] {
            if let Some(p) = vbr_in_dir(&PathBuf::from(&td).join(profile)) {
                return Some(p);
            }
        }
    }

    let mut roots = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(p) = vbr_in_dir(dir) {
                return Some(p);
            }
            let mut dir = dir.to_path_buf();
            for _ in 0..8 {
                roots.push(dir.clone());
                if !dir.pop() {
                    break;
                }
            }
        }
    }
    if let Ok(mut dir) = env::current_dir() {
        for _ in 0..8 {
            roots.push(dir.clone());
            if !dir.pop() {
                break;
            }
        }
    }
    // tide/app → Bust repo root (this source tree).
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    roots.push(manifest.join("../.."));
    roots.push(manifest.join("../../.."));

    for root in roots {
        if let Some(p) = vbr_in_target(&root) {
            return Some(p);
        }
    }
    None
}

/// Decide `vbr run` vs `vbr runproject`.
pub fn run_command_for(path: &Path) -> Result<(String, Vec<String>), String> {
    let bin = vbr_bin();
    if path.is_dir() {
        return Ok((bin, vec!["runproject".into(), path.display().to_string()]));
    }
    if let Some(parent) = path.parent() {
        let main = parent.join("main.vbr");
        if main.is_file() && path.file_name().is_some_and(|n| n != "main.vbr") {
            // Editing a module in a project folder — run the project.
            if parent
                .read_dir()
                .map(|mut d| {
                    d.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "vbr")))
                })
                .unwrap_or(false)
            {
                return Ok((bin, vec!["runproject".into(), parent.display().to_string()]));
            }
        }
        if path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("main.vbr"))
        {
            return Ok((bin, vec!["runproject".into(), parent.display().to_string()]));
        }
    }
    Ok((bin, vec!["run".into(), path.display().to_string()]))
}

fn banner_bin(bin: &str) -> &str {
    Path::new(bin)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(bin)
}

/// Leave alternate screen, run the child on the real terminal, come back.
/// Returns a short status line for the message area.
pub fn run_vbr(terminal: &mut TideTerminal, path: &Path) -> io::Result<String> {
    let (bin, args) = match run_command_for(path) {
        Ok(c) => c,
        Err(e) => return Ok(e),
    };
    let shown = banner_bin(&bin);

    // Suspend TUI
    terminal.clear()?;
    ratatui::restore();
    disable_raw_mode()?;

    println!();
    println!("── TIDE: {shown} {} ──", args.join(" "));
    println!();
    let _ = io::stdout().flush();

    let status = Command::new(&bin)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    if let Err(e) = &status {
        eprintln!("✘ Failed to launch `{bin}`: {e}");
        eprintln!(
            "  Tide needs the vbr CLI. From the Bust repo root:  cargo build\n  \
             or set VBR_BIN to the vbr executable (it is not on PATH)."
        );
    }

    print!("\n── Press Enter to return to TIDE ──\n");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);

    // Restore TUI
    enable_raw_mode()?;
    *terminal = ratatui::init();
    terminal.clear()?;

    match status {
        Ok(s) if s.success() => Ok(format!(
            "Program finished OK (`{shown} {}`).",
            args.join(" ")
        )),
        Ok(s) => Ok(format!(
            "Program exited with status {} (`{shown} {}`).",
            s.code().unwrap_or(-1),
            args.join(" ")
        )),
        Err(e) => Ok(format!(
            "Failed to run `{shown}`: {e}. Build vbr (`cargo build` in the Bust repo) or set VBR_BIN."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_vbr_in_a_cargo_target_dir() {
        let root = std::env::temp_dir().join(format!("tide_vbr_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let bin_dir = root.join("target").join("debug");
        fs::create_dir_all(&bin_dir).unwrap();
        let bin = bin_dir.join("vbr");
        fs::write(&bin, b"").unwrap();

        let found = vbr_in_target(&root).expect("should find target/debug/vbr");
        assert_eq!(found, bin);

        let _ = fs::remove_dir_all(&root);
    }
}
