// Functions, parameters and returns

fn main() {
    let a: i32 = add(2, 3);
    let s: i32 = square(4);
    let f: i32 = factorial(5);
    println!("{}", format!("{}{}", "2 + 3 = ", a));
    println!("{}", format!("{}{}", "4 squared = ", s));
    println!("{}", format!("{}{}", "5! = ", f));
}

fn add(x: i32, y: i32) -> i32 {
    x + y
}

fn square(n: i32) -> i32 {
    // VB style: assign to the function name
    n * n
}

fn factorial(n: i32) -> i32 {
    if n <= 1 {
        return 1;
    }
    n * factorial(n - 1)
}
