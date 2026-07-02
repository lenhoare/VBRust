// Built-in maths functions (work on floating-point values)

fn main() {
    let x: f64 = 9.0;
    let angle: f64 = 0.0;
    println!("sqrt(9)    = {}", x.sqrt());
    println!("abs(-5)    = {}", (-5.0f64).abs());
    println!("9 ^ 2      = {}", x.powi(2));
    println!("9 ^ 0.5    = {}", x.powf(0.5));
    println!("int(3.7)   = {}", 3.7f64.floor());
    println!("round(3.5) = {}", 3.5f64.round());
    println!("sin(0)     = {}", angle.sin());
    println!("cos(0)     = {}", angle.cos());
    println!("exp(1)     = {}", 1.0f64.exp());
    println!("ln(e)      = {}", 2.718281828f64.ln());
    // Mod gives the remainder (→ Rust's %, same precedence as * and /)
    let n: i64 = 17;
    println!("17 Mod 5   = {}", n % 5);
}
