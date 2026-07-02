// A String parameter defaults to ByVal — a read-only borrow. No keyword is
// needed to read it; only changing it requires ByRef.

fn loudly(message: &str) -> String {
    format!("{}!", message)
}

fn main() {
    let note: String = "hello".to_string();
    println!("{}", loudly(&note));
    println!("{}", note);
    // note is untouched — Loudly only borrowed it
}
