// Text … End Text — a multi-line string literal. `Text` alone at the end of a
// line opens the block; everything until `End Text` is the string, VERBATIM:
// quotes, backslashes, braces — nothing is ever escaped. The block dedents to
// its shallowest line, so it indents with your code without the indentation
// leaking into the string. Blank lines survive; there is no trailing newline.

fn main() {
    // The killer use: JSON bodies and SQL — no ""quote doubling"", no escapes.
    let body: String = "{\"model\": \"llama3\",\n \"prompt\": \"say hello\",\n \"stream\": false}".to_string();
    println!("{}", body);
    let sql: String = "SELECT name, score\nFROM ideas\n\nORDER BY score DESC".to_string();
    println!("{}", sql);
    // It's an ordinary string from every other angle — compose with `&`.
    // (Backslashes stay literal, as in every VBR string: C:\new\table.)
    let who: String = "world".to_string();
    println!("hello from\nC:\\new\\table -> {}", who);
}
