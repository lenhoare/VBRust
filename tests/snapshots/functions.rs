// Functions, parameters and returns

fn main() {
    let a: i64 = add(2, 3);
    let s: i64 = square(4);
    let f: i64 = factorial(5);
    println!("2 + 3 = {}", a);
    println!("4 squared = {}", s);
    println!("5! = {}", f);
}

fn add(x: i64, y: i64) -> i64 {
    x + y
}

fn square(n: i64) -> i64 {
    // VB style: assign to the function name
    n * n
}

fn factorial(n: i64) -> i64 {
    if n <= 1 {
        return 1;
    }
    n * factorial(n - 1)
}
