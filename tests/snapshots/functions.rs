// Functions, parameters and returns

fn vbr_main() -> Result<(), String> {
    let a: i64 = add(2, 3)?;
    let s: i64 = square(4)?;
    let f: i64 = factorial(5)?;
    println!("2 + 3 = {}", a);
    println!("4 squared = {}", s);
    println!("5! = {}", f);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn add(x: i64, y: i64) -> Result<i64, String> {
    Ok(x + y)
}

fn square(n: i64) -> Result<i64, String> {
    // VB style: assign to the function name
    Ok(n * n)
}

fn factorial(n: i64) -> Result<i64, String> {
    if n <= 1 {
        return Ok(1);
    }
    Ok(n * factorial(n - 1)?)
}
