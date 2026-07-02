// Rust Vec methods pass through alongside the VB style. The Option-returning
// accessors (first/last/get) pair naturally with Match over Some/None —
// the safe way to reach into a list.

fn main() {
    let mut nums: Vec<i64> = Vec::new();
    nums.push(3);
    nums.push(1);
    nums.push(2);
    match nums.first() {
        Some ( v ) => {
            println!("first  = {}", v);
        }
        None => {
            println!("empty");
        }
    }
    match nums.get(5) {
        Some ( v ) => {
            println!("at 5   = {}", v);
        }
        None => {
            println!("no index 5");
        }
    }
    // len() is a usize; assigning to a Long inserts the cast for you.
    let count: i64 = nums.len() as i64;
    println!("count  = {}", count);
    // contains takes &T on a Vec — the borrow is added for you.
    if nums.contains(&2) {
        println!("has 2");
    }
    // join builds a String from a Vec<String>.
    let mut words: Vec<String> = Vec::new();
    words.push("Ada".to_string());
    words.push("Grace".to_string());
    println!("names  = {}", words.join(" & "));
}
