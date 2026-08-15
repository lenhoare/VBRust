// A fallible action that returns no value on success. RaiseError fails; a
// bare End Function is success. Intercept at the call with Handle.

fn save(ok: bool) -> Result<(), String> {
    if !ok {
        return Err("save failed".to_string());
    }
    Ok(())
}

fn vbr_main() -> Result<(), String> {
    if let Err(e) = save(true) {
        println!("error: {}", e);
        return Ok(());
    }
    println!("saved");
    if let Err(e) = save(false) {
        println!("error: {}", e);
        return Ok(());
    }
    println!("saved");
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
