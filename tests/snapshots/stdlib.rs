// Standard library — file I/O and regex. Calls translate `.` to `::`.

use vbr_stdlib::{FileSystem, Regex};

fn main() {
    FileSystem::write("greeting.txt", "Hello   from   VBR").unwrap();
    let text: String = FileSystem::read("greeting.txt").unwrap();
    println!("{}", format!("{}{}", "file says: ", text));
    let cleaned: String = Regex::replace_all("\\s+", &text, " ").unwrap();
    println!("{}", format!("{}{}", "cleaned:   ", cleaned));
    FileSystem::delete("greeting.txt").unwrap();
}
