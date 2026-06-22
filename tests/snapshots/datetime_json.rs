// Standard library — DateTime and Json are value types you hold and use

use vbr_stdlib::{Json, DateTime};

fn main() {
    let now: DateTime = DateTime::now();
    println!("{}", format!("{}{}", "this year: ", now.year()));
    let later: DateTime = now.add_days(30);
    println!("{}", format!("{}{}", "in 30 days: ", later.format("%Y-%m-%d")));
    let person: Json = Json::parse("{\"name\":\"Alice\",\"age\":42}").unwrap();
    println!("{}", format!("{}{}", "name = ", person.get_string("name").unwrap()));
    println!("{}", format!("{}{}", "age  = ", person.get_int("age").unwrap()));
    let doc: Json = Json::parse("{\"tags\":[\"red\",\"green\",\"blue\"]}").unwrap();
    let tags: Vec<Json> = doc.get_array("tags").unwrap();
    println!("{}", format!("{}{}", "tag count: ", tags.len()));
    for tag in &tags {
        println!("{}", format!("{}{}", "  tag: ", (*tag).as_string().unwrap()));
    }
}
