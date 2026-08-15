// Standard library — DateTime and Json are value types you hold and use

use vbr_stdlib::{Json, DateTime};

fn vbr_main() -> Result<(), String> {
    let now: DateTime = DateTime::now();
    println!("this year: {}", now.year());
    let later: DateTime = now.add_days(30);
    println!("in 30 days: {}", later.format("%Y-%m-%d"));
    let person: Json = Json::parse("{\"name\":\"Alice\",\"age\":42}")?;
    println!("name = {}", person.get_string("name")?);
    println!("age  = {}", person.get_int("age")?);
    let doc: Json = Json::parse("{\"tags\":[\"red\",\"green\",\"blue\"]}")?;
    let tags: Vec<Json> = doc.get_array("tags")?;
    println!("tag count: {}", tags.len());
    for tag in &tags {
        println!("  tag: {}", (*tag).as_string()?);
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
