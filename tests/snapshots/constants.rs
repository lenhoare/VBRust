// Constants — module level, SCREAMING_SNAKE_CASE

const MAX_RETRIES: i64 = 3;
const GREETING: &str = "Hello";
pub const VERSION: f64 = 1.5;

fn vbr_main() -> Result<(), String> {
    let mut i: i64 = 0;
    while i < MAX_RETRIES {
        println!("{}, attempt {}", GREETING, i + 1);
        i = i + 1;
    }
    println!("version {}", VERSION);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
