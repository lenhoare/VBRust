// A multifile project: main.vbr calls into the shapes module.

mod shapes;

fn main() {
    let r: f64 = 3.0;
    println!("area:      {}", crate::shapes::circle_area(r));
    println!("perimeter: {}", crate::shapes::circle_perimeter(r));
}
