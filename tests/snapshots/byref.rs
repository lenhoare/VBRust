// ByRef parameters — the resolver inserts &mut at the call site and
// dereferences the parameter inside the function.

fn vbr_main() -> Result<(), String> {
    let mut total: i64 = 0;
    addto(&mut total, 5)?;
    addto(&mut total, 10)?;
    println!("total = {}", total);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn addto(target: &mut i64, amount: i64) -> Result<(), String> {
    *target = *target + amount;
    Ok(())
}
