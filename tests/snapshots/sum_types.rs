// Data-carrying enums (sum types): each variant carries its own data. Build one
// with `Shape.Circle(r)`; pull the data back out by matching. This is the same
// shape as Option/Result — now you can define your own.

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Empty,
}
impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn area(s: &Shape) -> Result<f64, String> {
    match s {
        Shape :: Circle ( r ) => {
            return Ok(3.14159 * r * r);
        }
        Shape :: Rectangle ( w , h ) => {
            return Ok(w * h);
        }
        Shape :: Empty => {
            return Ok(0.0);
        }
    }
}

fn vbr_main() -> Result<(), String> {
    let c: Shape = Shape::Circle(2.0);
    let r: Shape = Shape::Rectangle(3.0, 4.0);
    println!("circle area = {}", area(&c)?);
    println!("rect area   = {}", area(&r)?);
    println!("empty area  = {}", area(&Shape::Empty)?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
