// A multifile project: main.vbr calls into the shapes module.

mod shapes;

fn vbr_main() -> Result<(), String> {
    let r: f64 = 3.0;
    println!("area:      {}", crate::shapes::circlearea(r)?);
    println!("perimeter: {}", crate::shapes::circleperimeter(r)?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
