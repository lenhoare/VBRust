// Result<T> for fallible functions — propagate with ?, handle with Match

fn main() {
    // Handle the outcome explicitly
    match divide(10, 2) {
        Ok ( value ) => {
            println!("10 / 2 = {}", value);
        }
        Err ( message ) => {
            println!("error: {}", message);
        }
    }
    match divide(7, 0) {
        Ok ( value ) => {
            println!("7 / 0 = {}", value);
        }
        Err ( message ) => {
            println!("error: {}", message);
        }
    }
    // A function that uses ? to propagate failure
    match doublequotient(20, 4) {
        Ok ( value ) => {
            println!("double of 20 / 4 = {}", value);
        }
        Err ( message ) => {
            println!("error: {}", message);
        }
    }
    // .Unwrap() is allowed, but training wheels
    let known: i64 = divide(9, 3).unwrap();
    println!("9 / 3 = {}", known);
}

fn divide(numerator: i64, denominator: i64) -> Result<i64, String> {
    if denominator == 0 {
        return Err("cannot divide by zero".to_string());
    }
    Ok(numerator / denominator)
}

fn doublequotient(a: i64, b: i64) -> Result<i64, String> {
    let q: i64 = divide(a, b)?;
    Ok(q * 2)
}
