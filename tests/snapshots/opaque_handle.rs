// Opaque handles — hold a Rust value Bust has no type for, and pass it
// back into later Rust blocks. Here: a Rust iterator, held across blocks.

fn vbr_main() -> Result<(), String> {
    // No `As` — the type (a SplitWhitespace iterator) lives only in Rust.
    #[allow(unused_mut)]
    let mut words = { "the quick brown fox".split_whitespace() };
    // The handle persists between blocks; each .next() advances the same one.
    let first: String = { words.next().unwrap().to_string() };
    let second: String = { words.next().unwrap().to_string() };
    println!("first:  {}", first);
    println!("second: {}", second);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
