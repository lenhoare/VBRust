// A mixed project: VBR calls into a hand-written Rust module (text.rs).

mod text;

fn main() {
    println!("{}", format!("{}{}", "shout:  ", crate::text::shout("hello")));
    println!("{}", format!("{}{}", "repeat: ", crate::text::repeat("ab", 3)));
}
