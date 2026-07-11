// Shell — VB6's `Shell`, grown up. Two verbs:
// Shell.Run(cmd)   — run through the system shell, WAIT, capture the output:
// Ok(stdout) on success, Err(stderr) on a nonzero exit.
// Shell.Start(cmd) — launch and DON'T wait (VB6's actual Shell semantics):
// you get a Process handle to check on or stop.
// Pipes and PATH work — the command line goes through sh -c / cmd /C.
// A background child: start it, peek at it, stop it. `Kill` on an already-dead
// process is a harmless no-op; `Wait` returns the exit code (-1 after a kill).

use vbr_stdlib::{Shell, Process};

fn main() {
    match Shell::run("echo hello from VBR") {
        Ok ( output ) => {
            println!("said: {}", output);
        }
        Err ( why ) => {
            println!("echo failed: {}", why);
        }
    }
    match Shell::run("ls /vbr/definitely/missing") {
        Ok ( output ) => {
            println!("{}", output);
        }
        Err ( _ ) => {
            println!("as expected, that failed");
        }
    }
    match runchild() {
        Ok ( code ) => {
            println!("child finished with exit code {}", code);
        }
        Err ( why ) => {
            println!("child failed: {}", why);
        }
    }
}

fn runchild() -> Result<i64, String> {
    let mut child: Process = Shell::start("sleep 2")?;
    std::thread::sleep(std::time::Duration::from_millis((100) as u64));
    // VB6's kernel32 Sleep, no Declare needed (milliseconds)
    println!("running: {}", child.is_running());
    child.kill();
    println!("after kill: {}", child.is_running());
    Ok(child.wait())
}
