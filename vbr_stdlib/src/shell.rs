//! Shell — run commands and manage child processes (VB6's `Shell`, grown up).
//!
//! Two verbs, matching the two things VB6 code did with `Shell`:
//!
//! - `Shell::run(cmd)` — run a command, **wait** for it, and capture its
//!   output: `Ok(stdout)` on success, `Err(stderr)` on a nonzero exit.
//! - `Shell::start(cmd)` — launch a command and **don't wait** (VB6's actual
//!   `Shell` semantics): you get a `Process` handle back to check on
//!   (`is_running`) or stop (`kill`). The child's stdin/stdout/stderr are
//!   detached — a background server can't scribble over a terminal UI.
//!
//! Commands go through the system shell (`sh -c` on Unix, `cmd /C` on
//! Windows), so pipes, redirects, and PATH lookup work the way a terminal
//! user expects.

use std::process::{Child, Command, Stdio};

pub struct Shell;

impl Shell {
    /// Run `cmd`, wait for it to finish, and capture its output.
    /// Success (exit code 0) → `Ok(stdout)`, trailing newline trimmed.
    /// Failure → `Err(stderr)` (or the exit status when stderr is empty).
    pub fn run(cmd: &str) -> Result<String, String> {
        let output = Self::command(cmd)
            .output()
            .map_err(|e| format!("could not run '{}': {}", cmd, e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim_end().to_string();
            if err.is_empty() {
                Err(format!("'{}' failed: {}", cmd, output.status))
            } else {
                Err(err)
            }
        }
    }

    /// Launch `cmd` and return immediately with a handle to the running
    /// process — VB6's `Shell`. The child is detached from this program's
    /// terminal (its output goes nowhere), so it can run behind a `Screen`.
    pub fn start(cmd: &str) -> Result<Process, String> {
        Self::command(cmd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(Process)
            .map_err(|e| format!("could not start '{}': {}", cmd, e))
    }

    /// The platform's shell, primed with `cmd` — pipes and PATH included.
    fn command(cmd: &str) -> Command {
        #[cfg(windows)]
        {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(cmd);
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = Command::new("sh");
            c.arg("-c").arg(cmd);
            c
        }
    }
}

/// A running child process, from `Shell::start`. Dropping the handle does
/// *not* stop the process (like VB6); call `kill` to stop it.
pub struct Process(Child);

impl Process {
    /// Stop the process (and reap it, so it doesn't linger as a zombie).
    /// Already exited is fine — killing a dead process is a no-op.
    pub fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }

    /// Is it still running? (Doesn't wait — a snapshot, fine in a UI event.)
    pub fn is_running(&mut self) -> bool {
        matches!(self.0.try_wait(), Ok(None))
    }

    /// Block until the process finishes and return its exit code
    /// (`-1` when the code is unknowable — e.g. killed by a signal).
    pub fn wait(&mut self) -> i64 {
        match self.0.wait() {
            Ok(status) => status.code().map(i64::from).unwrap_or(-1),
            Err(_) => -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_captures_stdout() {
        let out = Shell::run("echo hello").expect("echo should succeed");
        assert_eq!(out, "hello");
    }

    #[test]
    fn run_reports_failure() {
        let err = Shell::run("ls /definitely/not/a/path/vbr").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn start_is_running_kill() {
        let mut p = Shell::start("sleep 5").expect("sleep should start");
        assert!(p.is_running());
        p.kill();
        assert!(!p.is_running());
        // Killing again is a harmless no-op.
        p.kill();
    }

    #[test]
    fn wait_returns_exit_code() {
        let mut p = Shell::start("sh -c 'exit 3'").expect("should start");
        assert_eq!(p.wait(), 3);
    }
}
