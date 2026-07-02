// Enum variants can now carry any payload — structs, several values, even a
// `Vec` (which also lets an enum hold a collection of things).

#[derive(Debug, Clone)]
struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Shape {
    Dot(Point),
    Segment(Point, Point),
    Blob(Vec<Point>),
    Empty,
}

fn describe(s: &Shape) -> String {
    match s {
        Shape :: Dot ( p ) => {
            return format!("dot at {},{}", p.x, p.y);
        }
        Shape :: Segment ( a , b ) => {
            return format!("segment {} to {}", a.x, b.x);
        }
        Shape :: Blob ( pts ) => {
            return format!("blob of {} points", pts.len());
        }
        Shape :: Empty => {
            return "nothing".to_string();
        }
    }
}

fn main() {
    println!("{}", describe(&Shape::Dot(Point { x: 1.0, y: 2.0 })));
    println!("{}", describe(&Shape::Segment(Point { x: 1.0, y: 2.0 }, Point { x: 5.0, y: 6.0 })));
    let mut cloud: Vec<Point> = Vec::new();
    cloud.push(Point { x: 1.0, y: 2.0 });
    cloud.push(Point { x: 5.0, y: 6.0 });
    println!("{}", describe(&Shape::Blob(cloud)));
    println!("{}", describe(&Shape::Empty));
}
