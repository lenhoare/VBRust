// Rust string methods pass through alongside the VB functions — use whichever
// reads better. Methods lower to real Rust (`.Trim()` and `.trim()` both work).

fn main() {
    let name: String = "  Ada Lovelace  ".to_string();
    // VB muscle memory still works...
    println!("VB UCase:  {}", name.to_uppercase());
    // ...and so do Rust's methods, with more reach.
    let clean: String = name.trim().to_string();
    println!("trimmed:   [{}]", clean);
    println!("upper:     {}", clean.to_uppercase());
    println!("swapped:   {}", clean.replace("Ada", "Grace"));
    println!("has Love:  {}", clean.contains("Love"));
    println!("length:    {}", clean.len());
    // Chaining, and a chain assigned into a String (coercion still applies).
    let shout: String = name.trim().to_uppercase();
    println!("chain:     {}", shout);
    // A mutating method — receiver is made `mut` for you.
    let mut greeting: String = "Hello".to_string();
    greeting.push_str(", world");
    println!("built:     {}", greeting);
}
