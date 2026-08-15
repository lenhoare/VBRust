// Vec<T> — a growable list

fn vbr_main() -> Result<(), String> {
    let mut nums: Vec<i64> = Vec::new();
    nums.push(10);
    nums.push(20);
    nums.push(30);
    println!("count = {}", nums.len());
    let mut total: i64 = 0;
    for n in &nums {
        total = total + *n;
    }
    println!("total = {}", total);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
