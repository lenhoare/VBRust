// String and ownership demo

fn main() {
    let greeting: String = "Hello".to_string();
    // a literal is a fixed-size &str
    let view = &greeting;
    // borrow — no copy is made
    let combined: String = format!("{}, World", greeting);
    // concat makes an owned String
    let copy: String = combined.clone();
    // explicit owned copy
    println!("{}", view);
    println!("{}", combined);
    println!("{}", copy);
}
