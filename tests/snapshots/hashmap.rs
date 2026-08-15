// HashMap<K, V> — VBA's Scripting.Dictionary, done natively

use std::collections::HashMap;

fn vbr_main() -> Result<(), String> {
    let mut ages: HashMap<String, i64> = HashMap::new();
    ages.insert("Alice".to_string(), 30);
    ages.insert("Bob".to_string(), 25);
    println!("has Alice? {}", ages.contains_key("Alice"));
    println!("has Bob?   {}", ages.contains_key("Bob"));
    println!("has Carol? {}", ages.contains_key("Carol"));
    for (name, age) in &ages {
        println!("{} is {}", *name, *age);
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
