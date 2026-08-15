// Guards (`If`) and the `_` wildcard. A guard is a Rust match guard — the arm
// only fires when its condition is also true. `x` binds the matched value.

fn describe(n: i64) -> Result<String, String> {
    match n {
        0 => {
            return Ok("zero".to_string());
        }
        x if x < 0 => {
            return Ok("negative".to_string());
        }
        x if x > 100 => {
            return Ok("huge".to_string());
        }
        _ => {
            return Ok("ordinary".to_string());
        }
    }
}

fn vbr_main() -> Result<(), String> {
    println!("-3 is {}", describe(-3)?);
    println!("0 is {}", describe(0)?);
    println!("42 is {}", describe(42)?);
    println!("500 is {}", describe(500)?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
