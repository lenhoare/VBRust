// Every fixed-size type — these all copy freely

fn main() {
    let small: i32 = 42;
    let count: i64 = 100000;
    let huge: i64 = 9000000000;
    let pi: f32 = 3.14;
    let ratio: f64 = 2.5;
    let flag: bool = true;
    let letter: u8 = 65;
    println!("small  = {}", small);
    println!("count  = {}", count);
    println!("huge   = {}", huge);
    println!("pi     = {}", pi);
    println!("ratio  = {}", ratio);
    println!("flag   = {}", flag);
    println!("letter = {}", letter);
}
