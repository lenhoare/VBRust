// Built-in maths functions (work on floating-point values)

fn main() {
    let x: f64 = 9.0;
    let angle: f64 = 0.0;
    println!("{}", format!("{}{}", "sqrt(9)    = ", x.sqrt()));
    println!("{}", format!("{}{}", "abs(-5)    = ", (-5.0f64).abs()));
    println!("{}", format!("{}{}", "9 ^ 2      = ", x.powi(2)));
    println!("{}", format!("{}{}", "9 ^ 0.5    = ", x.powf(0.5)));
    println!("{}", format!("{}{}", "int(3.7)   = ", 3.7f64.floor()));
    println!("{}", format!("{}{}", "round(3.5) = ", 3.5f64.round()));
    println!("{}", format!("{}{}", "sin(0)     = ", angle.sin()));
    println!("{}", format!("{}{}", "cos(0)     = ", angle.cos()));
    println!("{}", format!("{}{}", "exp(1)     = ", 1.0f64.exp()));
    println!("{}", format!("{}{}", "ln(e)      = ", 2.718281828f64.ln()));
}
