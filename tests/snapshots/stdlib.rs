// Standard library — file I/O and regex. Calls translate `.` to `::`.

use vbr_stdlib::{FileSystem, Regex};

fn vbr_main() -> Result<(), String> {
    FileSystem::write("greeting.txt", "Hello   from   Bust")?;
    let text: String = FileSystem::read("greeting.txt")?;
    println!("file says: {}", text);
    let cleaned: String = Regex::replace_all("\\s+", &text, " ")?;
    println!("cleaned:   {}", cleaned);
    FileSystem::delete("greeting.txt")?;
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
