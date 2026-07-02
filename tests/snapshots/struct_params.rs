// Structs as function parameters and return values

#[derive(Debug, Clone)]
struct Point {
    pub x: i64,
    pub y: i64,
}

fn origin() -> Point {
    Point { x: 0, y: 0 }
}

fn distance_squared(a: &Point, b: &Point) -> i64 {
    let dx: i64 = a.x - b.x;
    let dy: i64 = a.y - b.y;
    dx * dx + dy * dy
}

fn move_right(p: &mut Point, by: i64) {
    p.x = p.x + by;
}

fn main() {
    let mut p: Point = Point { x: 3, y: 4 };
    let o: Point = origin();
    println!("dist squared = {}", distance_squared(&p, &o));
    move_right(&mut p, 10);
    println!("after move, x = {}", p.x);
}
