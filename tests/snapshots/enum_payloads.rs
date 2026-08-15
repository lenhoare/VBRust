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
impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn describe(s: Shape) -> Result<String, String> {
    match s {
        Shape :: Dot ( p ) => {
            return Ok(format!("dot at {},{}", p.x, p.y));
        }
        Shape :: Segment ( a , b ) => {
            return Ok(format!("segment {} to {}", a.x, b.x));
        }
        Shape :: Blob ( pts ) => {
            return Ok(format!("blob of {} points", pts.len()));
        }
        Shape :: Empty => {
            return Ok("nothing".to_string());
        }
    }
}

fn vbr_main() -> Result<(), String> {
    println!("{}", describe(Shape::Dot(Point { x: 1.0, y: 2.0 }))?);
    println!("{}", describe(Shape::Segment(Point { x: 1.0, y: 2.0 }, Point { x: 5.0, y: 6.0 }))?);
    let mut cloud: Vec<Point> = Vec::new();
    cloud.push(Point { x: 1.0, y: 2.0 });
    cloud.push(Point { x: 5.0, y: 6.0 });
    println!("{}", describe(Shape::Blob(cloud))?);
    println!("{}", describe(Shape::Empty)?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
