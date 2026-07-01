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

fn area(s: &Shape) -> f64 {
    match s {
        Shape :: Circle ( r ) => {
            return 3.14159 * r * r;
        }
        Shape :: Rectangle ( w , h ) => {
            return w * h;
        }
        Shape :: Empty => {
            return 0.0;
        }
    }
}

fn main() {
    let c: Shape = Shape::Circle(2.0);
    let r: Shape = Shape::Rectangle(3.0, 4.0);
    println!("{}", format!("{}{}", "circle area = ", area(&c)));
    println!("{}", format!("{}{}", "rect area   = ", area(&r)));
    println!("{}", format!("{}{}", "empty area  = ", area(&Shape::Empty)));
}
