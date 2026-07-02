// More numeric coercion: maths on integers, and Return values

fn string_length(s: &str) -> i64 {
    // usize -> Long, coerced on return
    s.len() as i64
}

fn main() {
    let n: i64 = 9;
    println!("sqrt of 9 = {}", (n as f64).sqrt());
    // (n as f64).sqrt()
    println!("len of hello = {}", string_length("hello"));
}
