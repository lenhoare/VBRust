// Result<()> — a fallible action that returns no value on success. `Ok(())` is
// the unit success; failure carries the error as usual.

fn save(ok: bool) -> Result<(), String> {
    if !ok {
        return Err("save failed".to_string());
    }
    Ok(())
}

fn main() {
    match save(true) {
        Ok ( _ ) => {
            println!("saved");
        }
        Err ( e ) => {
            println!("error: {}", e);
        }
    }
    match save(false) {
        Ok ( _ ) => {
            println!("saved");
        }
        Err ( e ) => {
            println!("error: {}", e);
        }
    }
}
