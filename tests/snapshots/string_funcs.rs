// Built-in string functions

fn main() {
    let s: &str = "Hello, World";
    println!("{}", format!("{}{}", "length:    ", s.len()));
    println!("{}", format!("{}{}", "upper:     ", s.to_uppercase()));
    println!("{}", format!("{}{}", "lower:     ", s.to_lowercase()));
    println!("{}", format!("{}{}", "left 5:    ", &s[..5]));
    println!("{}", format!("{}{}", "right 5:   ", &s[s.len() - 5..]));
    println!("{}", format!("{}{}", "mid 2,3:   ", &s[1..4]));
    println!("{}", format!("{}{}", "trimmed:   ", "   padded   ".trim()));
    println!("{}", format!("{}{}", "replaced:  ", s.replace("World", "Rust")));
    println!("{}", format!("{}{}", "str of 42: ", 42.to_string()));
}
