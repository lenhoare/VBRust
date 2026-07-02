// shapes.vbr → module `shapes`. Public functions are visible across modules.

const PI: f64 = 3.14159;

pub fn circlearea(radius: f64) -> f64 {
    PI * radius * radius
}

pub fn circleperimeter(radius: f64) -> f64 {
    2.0 * PI * radius
}
