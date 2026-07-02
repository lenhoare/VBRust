// `Use` declares an external crate dependency; an inline Rust block then uses it.
// A single `Use` needs the project build — run with: vbr runproject

fn main() {
    let roll: i64 = {
        use rand::Rng;
        rand::thread_rng().gen_range(1..=6)
    };
    println!("you rolled a {}", roll);
}
