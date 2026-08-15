//! Suspend the designer, run the Screen through Bust, come back.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;

use crate::emit::design_to_vbr;
use crate::model::Design;

type Term = Terminal<CrosstermBackend<Stdout>>;

/// Same temp folder every Test so `build/` (and ratatui) stay cached.
fn test_dir() -> PathBuf {
    env::temp_dir().join("tide_design_test")
}

pub fn vbr_bin() -> PathBuf {
    if let Ok(p) = env::var("VBR_BIN") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("vbr");
            if cand.is_file() {
                return cand;
            }
            let cand = dir.join("vbr.exe");
            if cand.is_file() {
                return cand;
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for rel in ["target/debug/vbr", "target/release/vbr"] {
        let cand = root.join(rel);
        if cand.is_file() {
            return cand;
        }
    }
    PathBuf::from("vbr")
}

/// Write the current design as `main.vbr`, leave the TUI, `vbr runproject`.
pub fn run_test(terminal: &mut Term, design: &Design) -> io::Result<String> {
    let dir = test_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("main.vbr");
    std::fs::write(&path, design_to_vbr(design))?;

    let bin = vbr_bin();
    execute!(io::stdout(), DisableMouseCapture)?;
    terminal.clear()?;
    ratatui::restore();
    disable_raw_mode()?;

    println!();
    println!("── tide_design: Test ──");
    println!("  {} runproject {}", bin.display(), dir.display());
    println!("  q quits the Screen.");
    println!();
    let _ = io::stdout().flush();

    let status = Command::new(&bin)
        .arg("runproject")
        .arg(&dir)
        .status();

    print!("\n── Press Enter to return to the designer ──\n");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);

    enable_raw_mode()?;
    *terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    terminal.clear()?;

    Ok(status_message(&bin, &dir, status))
}

fn status_message(
    bin: &Path,
    dir: &Path,
    status: io::Result<std::process::ExitStatus>,
) -> String {
    match status {
        Ok(s) if s.success() => format!(
            " Test finished (`{} runproject {}`).",
            bin.display(),
            dir.display()
        ),
        Ok(s) => format!(
            " Test exited {} (`{} runproject`). Is vbr on PATH? (or set VBR_BIN)",
            s.code().unwrap_or(-1),
            bin.display()
        ),
        Err(e) => format!(
            " Failed to run `{}`: {e}. Put vbr on PATH or set VBR_BIN.",
            bin.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vbr_bin_is_nonempty() {
        assert!(!vbr_bin().as_os_str().is_empty());
    }
}
