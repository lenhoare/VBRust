// Several variables declared on one line — a common VB habit.
// Each variable carries its own `As Type`, with initialisers where wanted.

fn main() {
    let mut total: i64 = 0;
    let count: i32 = 3;
    let greeting: String = "items".to_string();
    let tag: String = "!".to_string();
    total = total + (count as i64);
    println!("{}: {}{}", greeting, total, tag);
}
