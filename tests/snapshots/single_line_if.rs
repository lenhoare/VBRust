// Single-line If: `If cond Then <stmt>` and `If cond Then <stmt> Else <stmt>`,
// with no `End If`. Block If still works as before.

fn sign(n: i64) -> Result<String, String> {
    if n < 0 {
        return Ok("negative".to_string());
    }
    if n == 0 {
        return Ok("zero".to_string());
    } else {
        return Ok("positive".to_string());
    }
}

fn vbr_main() -> Result<(), String> {
    let x: i64 = 5;
    if x > 3 {
        println!("big");
    } else {
        println!("small");
    }
    println!("-2 -> {}", sign(-2)?);
    println!(" 0 -> {}", sign(0)?);
    println!(" 7 -> {}", sign(7)?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
