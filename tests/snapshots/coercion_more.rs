// More numeric coercion: maths on integers, and Return values

fn stringlength(s: &str) -> Result<i64, String> {
    // usize -> Long, coerced on return
    Ok(s.len() as i64)
}

fn vbr_main() -> Result<(), String> {
    let n: i64 = 9;
    println!("sqrt of 9 = {}", (n as f64).sqrt());
    // (n as f64).sqrt()
    println!("len of hello = {}", stringlength("hello")?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
