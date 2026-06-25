// String ownership coercions: VBR inserts `.to_string()` wherever an owned String
// is expected but a &str is supplied — Ok(...) payloads, Vec<String>.push, Mid
// results, and assigning a literal to a String variable.

fn validate(name: &str) -> Result<String, String> {
    Ok(name.to_string())
}

fn first_char(text: &str) -> String {
    (&text[0..1]).to_string()
}

fn main() {
    let mut names: Vec<String> = Vec::new();
    names.push("Alice".to_string());
    names.push("Bob".to_string());
    let mut current: String = "start".to_string();
    println!("{}", format!("{}{}", "current     : ", current));
    current = "".to_string();
    println!("{}", format!("{}{}", format!("{}{}", "cleared     : [", current), "]"));
    current = "reset".to_string();
    println!("{}", format!("{}{}", "current     : ", current));
    let ch: String = (&"hello"[1..2]).to_string();
    println!("{}", format!("{}{}", "first char  : ", first_char("world")));
    println!("{}", format!("{}{}", "ch          : ", ch));
    println!("{}", format!("{}{}", "names count : ", names.len()));
    match validate("Ada") {
        Ok(v) => {
            println!("{}", format!("{}{}", "validated   : ", v));
        }
        Err(e) => {
            println!("{}", format!("{}{}", "err         : ", e));
        }
    }
}
