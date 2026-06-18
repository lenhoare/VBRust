// Option<T> for maybe-absent values — Some / None

fn main() {
    match halve(10) {
        Some(value) => {
            println!("{}", format!("{}{}", "half of 10 = ", value));
        }
        None => {
            println!("{}", "10 is odd, no exact half");
        }
    }
    match halve(7) {
        Some(value) => {
            println!("{}", format!("{}{}", "half of 7 = ", value));
        }
        None => {
            println!("{}", "7 is odd, no exact half");
        }
    }
}

fn halve(n: i32) -> Option<i32> {
    if n / 2 * 2 == n {
        return Some(n / 2);
    }
    None
}
