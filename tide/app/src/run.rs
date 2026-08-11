//! Suspend the TUI, run `vbr`, restore — Turbo Pascal compile-and-run loop.

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;

type TideTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn vbr_bin() -> String {
    env::var("VBR_BIN").unwrap_or_else(|_| "vbr".into())
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
                .map(|mut d| d.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "vbr"))))
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

/// Leave alternate screen, run the child on the real terminal, come back.
/// Returns a short status line for the message area.
pub fn run_vbr(terminal: &mut TideTerminal, path: &Path) -> io::Result<String> {
    let (bin, args) = match run_command_for(path) {
        Ok(c) => c,
        Err(e) => return Ok(e),
    };

    // Suspend TUI
    terminal.clear()?;
    ratatui::restore();
    disable_raw_mode()?;

    println!();
    println!("── TIDE: {bin} {} ──", args.join(" "));
    println!();
    let _ = io::stdout().flush();

    let status = Command::new(&bin).args(&args).status();

    print!("\n── Press Enter to return to TIDE ──\n");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);

    // Restore TUI
    enable_raw_mode()?;
    *terminal = ratatui::init();
    terminal.clear()?;

    match status {
        Ok(s) if s.success() => Ok(format!("Program finished OK (`{bin} {}`).", args.join(" "))),
        Ok(s) => Ok(format!(
            "Program exited with status {} (`{bin} {}`).",
            s.code().unwrap_or(-1),
            args.join(" ")
        )),
        Err(e) => Ok(format!(
            "Failed to run `{bin}`: {e}. Is vbr on PATH? (or set VBR_BIN)"
        )),
    }
}
