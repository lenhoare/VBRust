// `Log <message>` writes a timestamped line to `vbr.log` in the working
// directory — a diagnostic channel separate from `Debug.Print`. `Debug.Print`
// goes to the screen (fine for a console program); `Log` goes to a file, so it's
// safe *everywhere*, including inside a `Screen` where printing would scribble
// over the terminal UI. Watch a running app live with `tail -f build/vbr.log`.
// 
// A bare `Log` is INFO; `Log.Debug` / `Log.Warn` / `Log.Error` tag the severity,
// so you can `grep WARN build/vbr.log`. `Log` composes with `&` like
// `Debug.Print` and is available in any function, method, or surface event.
// (`Log(x)` with parentheses is still the natural-log builtin — the logging verb
// takes its message with a space, as `Debug.Print` does.)

fn vbr_log(level: &str, msg: &str) {
    use std::io::Write;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let ts = format!(
        "{:02}:{:02}:{:02}.{:03}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        now.subsec_millis()
    );
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("vbr.log") {
        let _ = writeln!(f, "[{} {}] {}", ts, level, msg);
    }
}

fn tally(items: &Vec<i64>) -> i64 {
    let mut total: i64 = 0;
    for n in &*items {
        total = total + *n;
        vbr_log("DEBUG", &format!("added {}, running total {}", *n, total));
    }
    if total == 0 {
        vbr_log("WARN ", "tally is empty");
    }
    vbr_log("INFO ", &format!("done — final total {}", total));
    total
}

fn main() {
    let nums: Vec<i64> = vec![3, 5, 8];
    let sum: i64 = tally(&nums);
    println!("sum = {}", sum);
}
