// Errors propagate automatically. Intercept a call with Handle; fail with RaiseError.

fn vbr_main() -> Result<(), String> {
    #[allow(unused_mut)]
    let mut value: i64;
    value = match divide(10, 2) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(message) => {
            println!("error: {}", message);
            return Ok(());
        }
    };
    println!("10 / 2 = {}", value);
    #[allow(unused_mut)]
    let mut bad: i64;
    bad = match divide(7, 0) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(message) => {
            println!("error: {}", message);
            return Ok(());
        }
    };
    println!("7 / 0 = {}", bad);
    // Failure from Divide flows out of DoubleQuotient with no extra syntax
    #[allow(unused_mut)]
    let mut doubled: i64;
    doubled = match doublequotient(20, 4) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(message) => {
            println!("error: {}", message);
            return Ok(());
        }
    };
    println!("double of 20 / 4 = {}", doubled);
    let known: i64 = divide(9, 3)?;
    println!("9 / 3 = {}", known);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn divide(numerator: i64, denominator: i64) -> Result<i64, String> {
    if denominator == 0 {
        return Err("cannot divide by zero".to_string());
    }
    Ok(((numerator as f64) / (denominator as f64)) as i64)
}

fn doublequotient(a: i64, b: i64) -> Result<i64, String> {
    let q: i64 = divide(a, b)?;
    Ok(q * 2)
}
