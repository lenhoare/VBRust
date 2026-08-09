// `If <expr> Is <pattern> Then …` — VB-flavoured `if let`. Handle just the one
// case you care about (usually Some/Ok) and skip the rest, without a full Match.
// `Is` is VB's word (as in VB6's `Is Nothing`); the Rust backend emits a real
// `if let`, so you see the idiomatic construct.

fn findprice(item: &str) -> Option<i64> {
    if item == "apple" {
        return Some(30);
    }
    None
}

fn main() {
    if let Some ( price ) = findprice("apple") {
        println!("apple costs {}", price);
    }
    if let Some ( price ) = findprice("pear") {
        println!("pear costs {}", price);
        // never runs — pear has no price
    }
    // Single-line form, too.
    if let Some ( price ) = findprice("apple") {
        println!("again: {}", price);
    }
}
