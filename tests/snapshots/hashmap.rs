// HashMap<K, V> — VBA's Scripting.Dictionary, done natively

use std::collections::HashMap;

fn main() {
    let mut ages: HashMap<String, i32> = HashMap::new();
    ages.insert("Alice".to_string(), 30);
    ages.insert("Bob".to_string(), 25);
    println!("{}", format!("{}{}", "Alice is ", ages.get("Alice").unwrap()));
    println!("{}", format!("{}{}", "has Bob?   ", ages.contains_key("Bob")));
    println!("{}", format!("{}{}", "has Carol? ", ages.contains_key("Carol")));
    for (name, age) in &ages {
        println!("{}", format!("{}{}", format!("{}{}", *name, " is "), *age));
    }
}
