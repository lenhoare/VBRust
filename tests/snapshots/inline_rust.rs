// Inline Rust — drop into Rust for a bit, get a plain value back

fn main() {
    let a: i64 = 5;
    let b: i64 = 7;
    // variables pass in automatically; last line (no semicolon) is the value
    let sum: i64 = { a + b };
    println!("sum is {}", sum);
    // a multi-line block — real Rust, sealed inside
    let big: i64 = {
        let mut total = 0;
        for i in 1..=100 {
            total += i;
        }
        total
    };
    println!("1 to 100 = {}", big);
}
