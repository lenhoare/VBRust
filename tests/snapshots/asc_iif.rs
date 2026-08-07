// Two classic VB builtins: Asc (the inverse of Chr) and IIf (immediate-if).
// Asc gives a character's code; IIf picks one of two values by a condition
// (lowered to a lazy Rust `if`/`else`, so — unlike VB — only one arm runs).

fn main() {
    println!("{}", "A".chars().next().map_or(0, |c| c as i64));
    // 65
    println!("{}", ((("A".chars().next().map_or(0, |c| c as i64) + 1) as u8) as char).to_string());
    // "B" — next letter
    let size: String = (if 10 > 3 { "big" } else { "small" }).to_string();
    println!("{}", size);
    let n: i64 = (if 4 % 2 == 0 { 100 } else { 200 }) as i64;
    println!("{}", n);
    // Mismatched arms (an owned String and a &str literal) still unify.
    let word: String = "hello".to_string();
    println!("{}", (if "z".chars().next().map_or(0, |c| c as i64) > 100 { word } else { "?".to_string() }));
}
