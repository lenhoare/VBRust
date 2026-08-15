// Compound assignment — +=, -=, *=, /= (a modern convenience over `a = a + 1`).

fn vbr_main() -> Result<(), String> {
    let mut n: i64 = 10;
    n += 5;
    n -= 3;
    n *= 2;
    n /= 4;
    println!("n = {}", n);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
