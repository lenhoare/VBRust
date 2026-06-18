// Every fixed-size type — these all copy freely

fn main() {
    let small: i16 = 42;
    let count: i32 = 100000;
    let huge: i64 = 9000000000;
    let pi: f32 = 3.14;
    let ratio: f64 = 2.5;
    let flag: bool = true;
    let letter: u8 = 65;
    let stamp: i64 = 0;
    println!("{}", format!("{}{}", "small  = ", small));
    println!("{}", format!("{}{}", "count  = ", count));
    println!("{}", format!("{}{}", "huge   = ", huge));
    println!("{}", format!("{}{}", "pi     = ", pi));
    println!("{}", format!("{}{}", "ratio  = ", ratio));
    println!("{}", format!("{}{}", "flag   = ", flag));
    println!("{}", format!("{}{}", "letter = ", letter));
    println!("{}", format!("{}{}", "stamp  = ", stamp));
}
