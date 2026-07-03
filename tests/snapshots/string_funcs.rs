// Built-in string functions

fn main() {
    let s: String = "Hello, World".to_string();
    println!("length:    {}", s.len());
    println!("upper:     {}", s.to_uppercase());
    println!("lower:     {}", s.to_lowercase());
    println!("left 5:    {}", s.chars().take(5).collect::<String>());
    println!("right 5:   {}", s.chars().skip(s.chars().count().saturating_sub(5)).collect::<String>());
    println!("mid 2,3:   {}", s.chars().skip(1).take(3).collect::<String>());
    println!("trimmed:   {}", "   padded   ".trim());
    println!("replaced:  {}", s.replace("World", "Rust"));
    println!("str of 42: {}", 42.to_string());
}
