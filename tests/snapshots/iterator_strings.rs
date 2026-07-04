// Iterator chains over OWNED elements — a Vec<String> clones items into the
// chain (`.iter().cloned()`, a real copy), filter's closure sees each item by
// reference, and find returns an Option you Match.

fn main() {
    let mut names: Vec<String> = Vec::new();
    names.push("Ada".to_string());
    names.push("Grace".to_string());
    names.push("Linus".to_string());
    let longnames: Vec<String> = names.iter().cloned().filter(|n| ((*n).len() as i32) > 3).collect();
    for n in &longnames {
        println!("long:  {}", *n);
    }
    let shouted: Vec<String> = names.iter().cloned().map(|n| n.to_uppercase()).collect();
    for n in &shouted {
        println!("loud:  {}", *n);
    }
    match names.iter().cloned().find(|n| (*n).starts_with("G")) {
        Some ( hit ) => {
            println!("found: {}", hit);
        }
        None => {
            println!("no match");
        }
    }
}
