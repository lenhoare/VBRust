// Standard library — DateTime and Json are value types you hold and use

use vbr_stdlib::{Json, DateTime};

fn main() {
    let now: DateTime = DateTime::now();
    println!("this year: {}", now.year());
    let later: DateTime = now.add_days(30);
    println!("in 30 days: {}", later.format("%Y-%m-%d"));
    let person: Json = Json::parse("{\"name\":\"Alice\",\"age\":42}").unwrap();
    println!("name = {}", person.get_string("name").unwrap());
    println!("age  = {}", person.get_int("age").unwrap());
    let doc: Json = Json::parse("{\"tags\":[\"red\",\"green\",\"blue\"]}").unwrap();
    let tags: Vec<Json> = doc.get_array("tags").unwrap();
    println!("tag count: {}", tags.len());
    for tag in &tags {
        println!("  tag: {}", (*tag).as_string().unwrap());
    }
}
