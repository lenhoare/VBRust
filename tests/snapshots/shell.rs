// Shell — VB6's `Shell`, grown up. Two verbs:
// Shell.Run(cmd)   — run through the system shell, WAIT, capture the output:
// stdout on success; failure propagates (or Handle it).
// Shell.Start(cmd) — launch and DON'T wait (VB6's actual Shell semantics):
// you get a Process handle to check on or stop.
// Pipes and PATH work — the command line goes through sh -c / cmd /C.
// A background child: start it, peek at it, stop it. `Kill` on an already-dead
// process is a harmless no-op; `Wait` returns the exit code (-1 after a kill).

use vbr_stdlib::{Shell, Process};

fn vbr_main() -> Result<(), String> {
    let mut output: String;
    output = match Shell::run("echo hello from Bust") {
        Ok(__vbr_ok) => __vbr_ok,
        Err(why) => {
            println!("echo failed: {}", why);
            return Ok(());
        }
    };
    println!("said: {}", output);
    if let Err(why) = Shell::run("ls /vbr/definitely/missing") {
        println!("as expected, that failed");
    }
    #[allow(unused_mut)]
    let mut code: i64;
    code = match runchild() {
        Ok(__vbr_ok) => __vbr_ok,
        Err(why) => {
            println!("child failed: {}", why);
            return Ok(());
        }
    };
    println!("child finished with exit code {}", code);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
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
