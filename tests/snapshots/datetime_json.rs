// Standard library — DateTime and Json are value types you hold and use

use vbr_stdlib::{Json, DateTime};

fn main() {
    let now: DateTime = DateTime::now();
    println!("this year: {}", now.year());
    let later: DateTime = now.adddays(30);
    println!("in 30 days: {}", later.format("%Y-%m-%d"));
    let person: Json = Json::parse("{\"name\":\"Alice\",\"age\":42}").unwrap();
    println!("name = {}", person.getstring("name").unwrap());
    println!("age  = {}", person.getint("age").unwrap());
    let doc: Json = Json::parse("{\"tags\":[\"red\",\"green\",\"blue\"]}").unwrap();
    let tags: Vec<Json> = doc.getarray("tags").unwrap();
    println!("tag count: {}", tags.len());
    for tag in &tags {
        println!("  tag: {}", (*tag).asstring().unwrap());
    }
}
