// ByRef parameters — the resolver inserts &mut at the call site and
// dereferences the parameter inside the function.

fn main() {
    let mut total: i32 = 0;
    add_to(&mut total, 5);
    add_to(&mut total, 10);
    println!("{}", format!("{}{}", "total = ", total));
}

fn add_to(target: &mut i32, amount: i32) {
    *target = *target + amount;
}
