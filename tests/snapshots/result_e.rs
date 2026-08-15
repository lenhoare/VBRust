// Typed errors are strings on the implicit Result<T, String> channel.
// RaiseError fails; Handle intercepts a call and binds the message.

fn safediv(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        return Err("div by zero".to_string());
    }
    if b < 0 {
        return Err("negative divisor".to_string());
    }
    Ok(((a as f64) / (b as f64)) as i32)
}

fn doublediv(a: i32, b: i32) -> Result<i32, String> {
    let q: i32 = safediv(a, b)?;
    Ok(q * 2)
}

fn vbr_main() -> Result<(), String> {
    #[allow(unused_mut)]
    let mut v: i32;
    v = match doublediv(10, 2) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(_) => {
            println!("failed");
            return Ok(());
        }
    };
    println!("ok: {}", v);
    #[allow(unused_mut)]
    let mut ignored: i32;
    ignored = match doublediv(10, 0) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(_) => {
            println!("failed");
            return Ok(());
        }
    };
    println!("ok: {}", ignored);
    #[allow(unused_mut)]
    let mut v3: i32;
    v3 = match doublediv(10, -2) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(err) => {
            println!("error: {}", err);
            return Ok(());
        }
    };
    println!("ok: {}", v3);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
