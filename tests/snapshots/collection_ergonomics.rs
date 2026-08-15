// Small ownership/coercion smoothings so Rust's rules don't leak into VB code:
// • a HashMap key that's a *variable* borrows just like a literal does — for
// both a `String` local and a ByVal `String` param (already a slice)
// • returning an element out of a Vec clones it (you can't move out of an index)
// • a Long constant assigned into a Double (or multiplied by a float) widens

use std::collections::HashMap;

pub const K: i64 = 32;

fn knows(scores: &HashMap<String, i64>, who: &str) -> Result<bool, String> {
    // `who` is a ByVal String param (already a `&str`) — no double borrow needed.
    Ok(scores.contains_key(who))
}

fn firstname(names: &Vec<String>) -> Result<String, String> {
    // Indexing a Vec can't *move* the String out — Bust clones it for you.
    Ok(names[0].clone())
}

fn vbr_main() -> Result<(), String> {
    let mut scores: HashMap<String, i64> = HashMap::new();
    scores.insert("Ada".to_string(), 95);
    // A `String` *local* key borrows for `contains_key`/`get`, like a literal.
    let who: String = "Ada".to_string();
    if scores.contains_key(&who) {
        println!("{} scored {}", who, scores.get(&who).copied().unwrap_or(0));
    }
    println!("known via param: {}", knows(&scores, &who)?);
    let names: Vec<String> = vec!["Ada".to_string(), "Grace".to_string()];
    println!("first is {}", firstname(&names)?);
    // A Long const widens into a Double, and mixes with floats.
    let weight: f64 = K as f64;
    let adjusted: f64 = weight + (K as f64) * 0.5;
    println!("weight {}, adjusted {}", weight, adjusted);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
