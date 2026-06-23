// `Sub` is sugar for a `Function` with no return value — both become a Rust `fn`.

fn greet(name: &str) {
    println!("{}", format!("{}{}", "Hello, ", name));
}

fn main() {
    greet("world");
}
