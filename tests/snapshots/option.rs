// Option<T> for maybe-absent values — Some / None

fn main() {
    match halve(10) {
        Some ( value ) => {
            println!("half of 10 = {}", value);
        }
        None => {
            println!("10 is odd, no exact half");
        }
    }
    match halve(7) {
        Some ( value ) => {
            println!("half of 7 = {}", value);
        }
        None => {
            println!("7 is odd, no exact half");
        }
    }
}

fn halve(n: i64) -> Option<i64> {
    if n % 2 == 0 {
        return Some(((n as f64) / (2 as f64)) as i64);
        // `/` floats; the Option<Long> payload narrows back to Long
    }
    None
}
