// A mixed project: VBR calls into a hand-written Rust module (text.rs).

mod text;

fn main() {
    println!("shout:  {}", crate::text::shout("hello"));
    println!("repeat: {}", crate::text::repeat("ab", 3));
}
