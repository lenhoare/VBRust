// Mid with a variable index (slice bounds cast to usize), and `date` usable as
// an ordinary identifier now that it's no longer a reserved type keyword.

fn main() {
    let s: String = "hello world".to_string();
    let start: i64 = 7;
    let length: i64 = 5;
    println!("{}", format!("{}{}", "mid(7,5)  : ", &s[((start - 1) as usize)..((start - 1 + length) as usize)]));
    println!("{}", format!("{}{}", "from(7)   : ", &s[((start - 1) as usize)..]));
    let date: String = "2026-06-25".to_string();
    println!("{}", format!("{}{}", "date      : ", date));
}
