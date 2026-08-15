// Structs as function parameters and return values

#[derive(Debug, Clone)]
struct Point {
    pub x: i64,
    pub y: i64,
}

fn origin() -> Result<Point, String> {
    Ok(Point { x: 0, y: 0 })
}

fn distancesquared(a: &Point, b: &Point) -> Result<i64, String> {
    let dx: i64 = a.x - b.x;
    let dy: i64 = a.y - b.y;
    Ok(dx * dx + dy * dy)
}

fn moveright(p: &mut Point, by: i64) -> Result<(), String> {
    p.x = p.x + by;
    Ok(())
}

fn vbr_main() -> Result<(), String> {
    let mut p: Point = Point { x: 3, y: 4 };
    let o: Point = origin()?;
    println!("dist squared = {}", distancesquared(&p, &o)?);
    moveright(&mut p, 10)?;
    println!("after move, x = {}", p.x);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
