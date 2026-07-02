// Guards (`If`) and the `_` wildcard. A guard is a Rust match guard — the arm
// only fires when its condition is also true. `x` binds the matched value.

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
    println!("-3 is {}", describe(-3));
    println!("0 is {}", describe(0));
    println!("42 is {}", describe(42));
    println!("500 is {}", describe(500));
}
