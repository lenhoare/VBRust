// Case guards (If) and the _ wildcard

fn describe(n: i64) -> String {
    match n {
        0 => {
            return "zero".to_string();
        }
        x if x < 0 => {
            return "negative".to_string();
        }
        x if x > 100 => {
            return "huge".to_string();
        }
        _ => {
            return "ordinary".to_string();
        }
    }
}

fn main() {
    println!("{}", format!("{}{}", "-3 is ", describe(-3)));
    println!("{}", format!("{}{}", "0 is ", describe(0)));
    println!("{}", format!("{}{}", "42 is ", describe(42)));
    println!("{}", format!("{}{}", "500 is ", describe(500)));
}
