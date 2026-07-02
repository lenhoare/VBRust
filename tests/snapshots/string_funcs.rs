// Built-in string functions

fn main() {
    let s: String = "Hello, World".to_string();
    println!("length:    {}", s.len());
    println!("upper:     {}", s.to_uppercase());
    println!("lower:     {}", s.to_lowercase());
    println!("left 5:    {}", &s[..5]);
    println!("right 5:   {}", &s[s.len() - 5..]);
    println!("mid 2,3:   {}", &s[1..4]);
    println!("trimmed:   {}", "   padded   ".trim());
    println!("replaced:  {}", s.replace("World", "Rust"));
    println!("str of 42: {}", 42.to_string());
}
