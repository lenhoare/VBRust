// Builtins with a teaching twist: Mid adjusts indexing, while InStr and Val
// return Option / Result. (Handling those Results comes in a later slice, so
// this file shows the lowering but is not compiled.)

fn main() {
    println!("{}", &"hello"[1..4]);
    let pos: i64 = "hello".find("l");
    let num: f64 = "3.14".parse::<f64>();
}
