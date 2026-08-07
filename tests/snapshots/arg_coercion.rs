// Call arguments get the same adaptation an assignment does, at every call site:
// an int literal or a Long variable widens to a `Double` param, a `Double` (`Val`)
// narrows to a `Long` param, and a String-returning call (`Mid`) borrows to feed a
// `&str` param. (A-series coercion cluster.)

pub fn ctof(c: f64) -> f64 {
    c * 9.0 / 5.0 + 32.0
}

pub fn charat(text: &str, pos: i64) -> String {
    text.chars().skip(((pos) - 1) as usize).take(1).collect::<String>()
}

pub fn findin(haystack: &str, needle: &str) -> i64 {
    match haystack.find(needle).map(|p| p as i64) {
        Some ( p ) => {
            return p;
        }
        None => {
            return -1;
        }
    }
}

fn main() {
    println!("{}", ctof(100.0));
    // int literal -> Double param
    for k in (0..=20).step_by(10) {
        println!("{}", ctof(k as f64));
        // Long counter -> Double param
    }
    let word: String = "hello".to_string();
    println!("{}", charat(&word, "2".trim().parse::<f64>().unwrap_or(0.0) as i64));
    // Val (Double) -> Long param
    println!("{}", findin(&word, &word.chars().skip(2).take(1).collect::<String>()));
    // Mid (String) -> &str param
}
