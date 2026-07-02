// Single-line If: `If cond Then <stmt>` and `If cond Then <stmt> Else <stmt>`,
// with no `End If`. Block If still works as before.

fn sign(n: i64) -> String {
    if n < 0 {
        return "negative".to_string();
    }
    if n == 0 {
        return "zero".to_string();
    } else {
        return "positive".to_string();
    }
}

fn main() {
    let x: i64 = 5;
    if x > 3 {
        println!("big");
    } else {
        println!("small");
    }
    println!("-2 -> {}", sign(-2));
    println!(" 0 -> {}", sign(0));
    println!(" 7 -> {}", sign(7));
}
