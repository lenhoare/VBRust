// VBR vertical-slice demo — everything here is in the first milestone

fn main() {
    let count: i32 = 3;
    let mut total: i32 = 0;
    for i in 1..=count {
        total = total + i;
    }
    let ratio: f64 = 2.5;
    println!("{}", format!("{}{}", format!("{}{}", format!("{}{}", "Sum 1..", count), " = "), total));
    println!("{}", format!("{}{}", "ratio is ", ratio));
    if total > 5 {
        println!("{}", "big");
    } else if total == 5 {
        println!("{}", "exactly five");
    } else {
        println!("{}", "small");
    }
}
