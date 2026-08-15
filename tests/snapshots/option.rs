// Option<T> for maybe-absent values — Some / None

fn vbr_main() -> Result<(), String> {
    match halve(10)? {
        Some ( value ) => {
            println!("half of 10 = {}", value);
        }
        None => {
            println!("10 is odd, no exact half");
        }
    }
    match halve(7)? {
        Some ( value ) => {
            println!("half of 7 = {}", value);
        }
        None => {
            println!("7 is odd, no exact half");
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn halve(n: i64) -> Result<Option<i64>, String> {
    if n % 2 == 0 {
        return Ok(Some(((n as f64) / (2 as f64)) as i64));
        // `/` floats; the Option<Long> payload narrows back to Long
    }
    Ok(None)
}
