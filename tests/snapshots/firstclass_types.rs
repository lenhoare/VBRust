// Result<T>, Option<T> and Vec<T> as first-class parameter and return types.
// Returns a Vec by value.
// Returns an Option by value.
// A nested wrapper: Result<Vec<String>>.

fn main() {
    let evens: Vec<i64> = evensupto(10);
    println!("evens count = {}", evens.len());
    match firstword("hello world") {
        Some ( w ) => {
            println!("first word = {}", w);
        }
        None => {
            println!("no words");
        }
    }
    match lines("a,b,c") {
        Ok ( parts ) => {
            println!("parts = {}", parts.len());
        }
        Err ( message ) => {
            println!("error: {}", message);
        }
    }
}

fn evensupto(limit: i64) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut n: i64 = 0;
    while n <= limit {
        result.push(n);
        n = n + 2;
    }
    result
}

fn firstword(text: &str) -> Option<String> {
    if (text.len() as i32) == 0 {
        return None;
    }
    Some(text.to_string())
}

fn lines(text: &str) -> Result<Vec<String>, String> {
    if (text.len() as i32) == 0 {
        return Err("empty input".to_string());
    }
    let mut parts: Vec<String> = Vec::new();
    parts.push(text.to_string());
    Ok(parts)
}
