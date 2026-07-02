// ByRef parameters — the resolver inserts &mut at the call site and
// dereferences the parameter inside the function.

fn main() {
    let mut total: i64 = 0;
    addto(&mut total, 5);
    addto(&mut total, 10);
    println!("total = {}", total);
}

fn addto(target: &mut i64, amount: i64) {
    *target = *target + amount;
}
