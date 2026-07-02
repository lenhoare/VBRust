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
    println!("current     : {}", current);
    current = "".to_string();
    println!("cleared     : [{}]", current);
    current = "reset".to_string();
    println!("current     : {}", current);
    let ch: String = (&"hello"[1..2]).to_string();
    println!("first char  : {}", first_char("world"));
    println!("ch          : {}", ch);
    println!("names count : {}", names.len());
    match validate("Ada") {
        Ok ( v ) => {
            println!("validated   : {}", v);
        }
        Err ( e ) => {
            println!("err         : {}", e);
        }
    }
}
