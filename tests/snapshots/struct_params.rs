// Structs as function parameters and return values

struct Point {
    pub x: i32,
    pub y: i32,
}

fn origin() -> Point {
    Point { x: 0, y: 0 }
}

fn distance_squared(a: &Point, b: &Point) -> i32 {
    let dx: i32 = a.x - b.x;
    let dy: i32 = a.y - b.y;
    dx * dx + dy * dy
}

fn move_right(p: &mut Point, by: i32) {
    p.x = p.x + by;
}

fn main() {
    let mut p: Point = Point { x: 3, y: 4 };
    let o: Point = origin();
    println!("{}", format!("{}{}", "dist squared = ", distance_squared(&p, &o)));
    move_right(&mut p, 10);
    println!("{}", format!("{}{}", "after move, x = ", p.x));
}
