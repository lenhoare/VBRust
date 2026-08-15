// Rust won't silently convert between number types — Bust inserts `as` for you.

fn vbr_main() -> Result<(), String> {
    let length: i64 = "hello world".len() as i64;
    // usize -> Long
    let ratio: f64 = length as f64;
    // Long  -> Double
    let small: i32 = length as i32;
    // Long  -> Integer (may narrow)
    println!("length = {}", length);
    println!("ratio  = {}", ratio);
    println!("small  = {}", small);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
