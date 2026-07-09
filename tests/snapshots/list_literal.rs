// Inline list literals — `[a, b, …]` builds a Vec<T>.
// 
// Prefix `[…]` is a list; postfix `x[i]` is still indexing — no clash, exactly
// like Rust. String elements are owned automatically; numbers take their type
// from the target.

fn total(xs: &Vec<i64>) -> i64 {
    let mut sum: i64 = 0;
    for x in &*xs {
        sum = sum + *x;
    }
    sum
}

fn main() {
    let names: Vec<String> = vec!["alice".to_string(), "bob".to_string(), "carol".to_string()];
    println!("first = {}, of {}", names[0], names.len());
    // A list literal passed straight into a function (the common case for, e.g.,
    // query parameters).
    println!("total = {}", total(&vec![10, 20, 30]));
    let empty: Vec<String> = vec![];
    println!("empty count = {}", empty.len());
}
