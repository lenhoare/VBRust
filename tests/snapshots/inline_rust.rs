// Inline Rust — drop into Rust for a bit, get a plain value back

fn main() {
    let a: i32 = 5;
    let b: i32 = 7;
    // variables pass in automatically; last line (no semicolon) is the value
    let sum: i32 = { a + b };
    println!("{}", format!("{}{}", "sum is ", sum));
    // a multi-line block — real Rust, sealed inside
    let big: i32 = {
        let mut total = 0;
        for i in 1..=100 {
            total += i;
        }
        total
    };
    println!("{}", format!("{}{}", "1 to 100 = ", big));
}
