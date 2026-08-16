// Split, Join, Space — VB's string-list builtins.

fn vbr_main() -> Result<(), String> {
    let csv: String = "one,two,three".to_string();
    let parts: Vec<String> = csv.split(",").map(|__p| __p.to_string()).collect::<Vec<_>>();
    println!("{}", parts.join(" / "));
    // Default delimiter is a single space, both ways.
    println!("{}", "a b c".split(' ').map(|__p| __p.to_string()).collect::<Vec<_>>().join(" "));
    println!("[{}]", " ".repeat(((3) as i64).max(0) as usize));
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
