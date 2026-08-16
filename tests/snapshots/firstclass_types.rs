// Result<T>, Option<T> and Vec<T> as first-class parameter and return types.
// Returns a Vec by value.
// Returns an Option by value.
// A nested wrapper: a function that returns Vec<String>, and can fail.

fn vbr_main() -> Result<(), String> {
    let evens: Vec<i64> = evensupto(10)?;
    println!("evens count = {}", evens.len());
    match firstword("hello world")? {
        Some ( w ) => {
            println!("first word = {}", w);
        }
        None => {
            println!("no words");
        }
    }
    let parts: Vec<String> = lines("a,b,c")?;
    println!("parts = {}", parts.len());
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn evensupto(limit: i64) -> Result<Vec<i64>, String> {
    let mut result: Vec<i64> = Vec::new();
    let mut n: i64 = 0;
    while n <= limit {
        result.push(n);
        n = n + 2;
    }
    Ok(result)
}

fn firstword(text: &str) -> Result<Option<String>, String> {
    if text.chars().count() as i64 == 0 {
        return Ok(None);
    }
    Ok(Some(text.to_string()))
}

fn lines(text: &str) -> Result<Vec<String>, String> {
    if text.chars().count() as i64 == 0 {
        return Err("empty input".to_string());
    }
    let mut parts: Vec<String> = Vec::new();
    parts.push(text.to_string());
    Ok(parts)
}
