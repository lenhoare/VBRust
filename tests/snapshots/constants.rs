// Constants — module level, SCREAMING_SNAKE_CASE

const MAX_RETRIES: i32 = 3;
const GREETING: &str = "Hello";
pub const VERSION: f64 = 1.5;

fn main() {
    let mut i: i32 = 0;
    while i < MAX_RETRIES {
        println!("{}", format!("{}{}", format!("{}{}", GREETING, ", attempt "), i + 1));
        i = i + 1;
    }
    println!("{}", format!("{}{}", "version ", VERSION));
}
