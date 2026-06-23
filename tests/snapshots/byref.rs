// ByRef parameters — the resolver inserts &mut at the call site and
// dereferences the parameter inside the function.

fn main() {
    let mut total: i64 = 0;
    add_to(&mut total, 5);
    add_to(&mut total, 10);
    println!("{}", format!("{}{}", "total = ", total));
}

fn add_to(target: &mut i64, amount: i64) {
    *target = *target + amount;
}
