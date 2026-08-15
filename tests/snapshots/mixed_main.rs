// A mixed project: Bust calls into a hand-written Rust module (text.rs).

mod text;

fn vbr_main() -> Result<(), String> {
    println!("shout:  {}", crate::text::shout("hello"));
    println!("repeat: {}", crate::text::repeat("ab", 3));
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
