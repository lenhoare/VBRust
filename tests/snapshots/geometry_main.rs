// A multifile project: main.vbr calls into the shapes module.

mod shapes;

fn main() {
    let r: f64 = 3.0;
    println!("area:      {}", crate::shapes::circlearea(r));
    println!("perimeter: {}", crate::shapes::circleperimeter(r));
}
