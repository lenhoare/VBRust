// Numeric/borrow coercions the transpiler inserts so real programs compile:
// integer ^ → .pow, Vec index/compare → usize casts, For Each over a borrowed
// Vec param, and .Clone() on a &str parameter → .to_string().

fn square(n: i64) -> Result<i64, String> {
    Ok(n.pow((2) as u32))
}

fn sumandfirst(nums: &Vec<i64>) -> Result<i64, String> {
    let mut total: i64 = 0;
    for x in &*nums {
        total += *x;
    }
    let i: i64 = 0;
    if i < (nums.len() as i64) {
        total += nums[(i) as usize];
    }
    Ok(total)
}

fn dup(s: &str) -> Result<String, String> {
    Ok(s.to_string())
}

fn vbr_main() -> Result<(), String> {
    let v: Vec<i64> = { vec![10, 20, 30] };
    println!("square(3)  = {}", square(3)?);
    println!("sum+first  = {}", sumandfirst(&v)?);
    println!("dup        = {}", dup("hi")?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
