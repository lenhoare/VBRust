// String ownership coercions: Bust inserts `.to_string()` wherever an owned String
// is expected but a &str is supplied — function returns, Vec<String>.push, Mid
// results, and assigning a literal to a String variable.

fn validate(name: &str) -> Result<String, String> {
    Ok(name.to_string())
}

fn firstchar(text: &str) -> Result<String, String> {
    Ok(text.chars().skip(0).take(1).collect::<String>())
}

fn vbr_main() -> Result<(), String> {
    let mut names: Vec<String> = Vec::new();
    names.push("Alice".to_string());
    names.push("Bob".to_string());
    let mut current: String = "start".to_string();
    println!("current     : {}", current);
    current = "".to_string();
    println!("cleared     : [{}]", current);
    current = "reset".to_string();
    println!("current     : {}", current);
    let ch: String = "hello".chars().skip(1).take(1).collect::<String>();
    println!("first char  : {}", firstchar("world")?);
    println!("ch          : {}", ch);
    println!("names count : {}", names.len());
    println!("validated   : {}", validate("Ada")?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
