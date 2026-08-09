// `If <expr> Is <pattern> Then …` — VB-flavoured `if let`. Handle just the one
// case you care about (usually Some/Ok), with an optional `Else`. `Is` is VB's
// word (as in VB6's `Is Nothing`). `Do While <expr> Is <pattern>` is `while let`:
// loop while the pattern keeps matching. The Rust backend emits the idiomatic
// `if let` / `loop { if let … else break }`.

fn findprice(item: &str) -> Option<i64> {
    if item == "apple" {
        return Some(30);
    }
    None
}

fn nextitem(xs: &mut Vec<i64>, idx: i64) -> Option<i64> {
    if idx < (xs.len() as i64) {
        return Some(xs[(idx) as usize]);
    }
    None
}

fn main() {
    if let Some ( price ) = findprice("apple") {
        println!("apple costs {}", price);
    } else {
        println!("no price for apple");
    }
    if let Some ( price ) = findprice("pear") {
        println!("pear costs {}", price);
    } else {
        println!("no price for pear");
    }
    // Single-line form.
    if let Some ( price ) = findprice("apple") {
        println!("again: {}", price);
    }
    // while let — drain a list of prices.
    let mut prices: Vec<i64> = vec![10, 20, 30];
    let mut i: i64 = 0;
    loop {
        if let Some ( v ) = nextitem(&mut prices, i) {
            println!("item {}", v);
            i = i + 1;
        } else {
            break;
        }
    }
}
