// Inside brackets — `(` `[` `{` — a newline is just whitespace (Python-style), so
// a list literal, a call, or a struct literal can span lines. A trailing comma is
// allowed. (A plain "…" string stays one line; use `Text … End Text` for
// multi-line text.)

#[derive(Debug, Clone)]
struct Point {
    pub x: i64,
    pub y: i64,
}

fn sum3(a: i64, b: i64, c: i64) -> i64 {
    a + b + c
}

fn main() {
    let art: Vec<String> = vec!["  /\\  ".to_string(), " /  \\ ".to_string(), "/____\\".to_string()];
    for row in &art {
        println!("{}", *row);
    }
    let total: i64 = sum3(10, 20, 30);
    println!("{}", total);
    let origin: Point = Point { x: 0, y: 0 };
    println!("{}", origin.x + origin.y);
}
