// Logical operators: And, Or, Not, Xor. Logical (short-circuit) and looser
// than comparison, just like Rust's &&, ||, !, ^ — no backwards-compat quirks.

fn main() {
    let age: i64 = 30;
    let member: bool = true;
    if age >= 18 && member {
        println!("{}", "admitted");
    }
    if age < 13 || age > 65 {
        println!("{}", "discounted");
    } else {
        println!("{}", "full price");
    }
    if !member {
        println!("{}", "please join");
    } else {
        println!("{}", "welcome back");
    }
    // Xor: true when exactly one side is true.
    let heads: bool = true;
    let tails: bool = false;
    println!("{}", format!("{}{}", "valid coin: ", heads ^ tails));
    // Precedence: And binds tighter than Or, comparisons tighter than both.
    let ok: bool = age > 0 && age < 120 || member;
    println!("{}", format!("{}{}", "ok: ", ok));
}
