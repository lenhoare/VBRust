// shapes.vbr → module `shapes`. Public functions are visible across modules.

const PI: f64 = 3.14159;

pub fn circlearea(radius: f64) -> Result<f64, String> {
    Ok(PI * radius * radius)
}

pub fn circleperimeter(radius: f64) -> Result<f64, String> {
    Ok(2.0 * PI * radius)
}
