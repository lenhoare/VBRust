// Rust won't silently convert between number types — VBR inserts `as` for you.

fn main() {
    let length: i32 = "hello world".len() as i32;
    // usize -> Long
    let ratio: f64 = length as f64;
    // Long  -> Double
    let small: i16 = length as i16;
    // Long  -> Integer (may narrow)
    println!("{}", format!("{}{}", "length = ", length));
    println!("{}", format!("{}{}", "ratio  = ", ratio));
    println!("{}", format!("{}{}", "small  = ", small));
}
