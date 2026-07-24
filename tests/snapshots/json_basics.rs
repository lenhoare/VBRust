// Json from the standard library — parse a document and read typed fields.

use vbr_stdlib::{Json};

fn main() {
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
