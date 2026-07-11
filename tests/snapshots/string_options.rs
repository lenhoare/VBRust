// Builtins with a teaching twist: Mid adjusts indexing, while InStr and Val
// return Option / Result. (Handling those Results comes in a later slice, so
// this file shows the lowering but is not compiled.)

fn main() {
    println!("{}", "hello".chars().skip(1).take(3).collect::<String>());
    let pos: i64 = "hello".find("l");
    let num: f64 = "3.14".trim().parse::<f64>().unwrap_or(0.0);
}
