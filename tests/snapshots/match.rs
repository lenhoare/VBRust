// Match → Rust's `match`. Each arm is `pattern => body`; the patterns are real
// Rust — literals, ranges (`..=`), alternation (`|`), and the `_` wildcard.

fn vbr_main() -> Result<(), String> {
    let score: i64 = 75;
    match score {
        100 => {
            println!("perfect");
        }
        90 ..= 99 => {
            println!("excellent");
        }
        70 ..= 89 => {
            println!("good");
        }
        0 | 1 | 2 => {
            println!("very low");
        }
        _ => {
            println!("somewhere in between");
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
