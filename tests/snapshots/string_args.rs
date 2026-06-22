// Passing strings to functions — VBR borrows an owned String automatically

fn shout(text: &str) -> String {
    text.to_uppercase()
}

fn main() {
    let name: String = "alice".to_string();
    println!("{}", shout(&name));
    // owned String  -> shout(&name)
    println!("{}", shout("bob"));
    // literal &str   -> shout("bob")
    println!("{}", shout(&format!("{}{}", name, "!")));
    // concat String  -> shout(&format!(...))
}
