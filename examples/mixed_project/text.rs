//! text.rs — a hand-written Rust module, called from VBR like any other.
//! It just skips the transpile step; the qualified-call machinery treats it
//! exactly like a .vbr module.

pub fn shout(s: &str) -> String {
    format!("{}!", s.to_uppercase())
}

pub fn repeat(s: &str, n: i32) -> String {
    s.repeat(n as usize)
}
