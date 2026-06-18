// Result<T> for fallible functions — propagate with ?, handle with Match

fn main() {
    // Handle the outcome explicitly
    match divide(10, 2) {
        Ok(value) => {
            println!("{}", format!("{}{}", "10 / 2 = ", value));
        }
        Err(message) => {
            println!("{}", format!("{}{}", "error: ", message));
        }
    }
    match divide(7, 0) {
        Ok(value) => {
            println!("{}", format!("{}{}", "7 / 0 = ", value));
        }
        Err(message) => {
            println!("{}", format!("{}{}", "error: ", message));
        }
    }
    // A function that uses ? to propagate failure
    match double_quotient(20, 4) {
        Ok(value) => {
            println!("{}", format!("{}{}", "double of 20 / 4 = ", value));
        }
        Err(message) => {
            println!("{}", format!("{}{}", "error: ", message));
        }
    }
    // .Unwrap() is allowed, but training wheels
    let known: i32 = divide(9, 3).unwrap();
    println!("{}", format!("{}{}", "9 / 3 = ", known));
}

fn divide(numerator: i32, denominator: i32) -> Result<i32, String> {
    if denominator == 0 {
        return Err("cannot divide by zero".to_string());
    }
    Ok(numerator / denominator)
}

fn double_quotient(a: i32, b: i32) -> Result<i32, String> {
    let q: i32 = divide(a, b)?;
    Ok(q * 2)
}
