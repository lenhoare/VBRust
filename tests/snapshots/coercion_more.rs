// More numeric coercion: maths on integers, and Return values

fn string_length(s: &str) -> i32 {
    // usize -> Long, coerced on return
    s.len() as i32
}

fn main() {
    let n: i32 = 9;
    println!("{}", format!("{}{}", "sqrt of 9 = ", (n as f64).sqrt()));
    // (n as f64).sqrt()
    println!("{}", format!("{}{}", "len of hello = ", string_length("hello")));
}
