// Rust number methods pass through too — same deal as strings. Each returns the
// receiver's own type, so casts and concatenation still line up.

fn main() {
    let n: i64 = -42;
    // Integer methods.
    println!("{}", format!("{}{}", "abs:     ", n.abs()));
    println!("{}", format!("{}{}", "max(0):  ", n.max(0)));
    println!("{}", format!("{}{}", "clamp:   ", n.clamp(-10, 10)));
    let base: i64 = 2;
    println!("{}", format!("{}{}", "2^10:    ", base.pow((10) as u32)));
    // Float methods.
    let x: f64 = 7.3;
    println!("{}", format!("{}{}", "sqrt:    ", x.sqrt()));
    println!("{}", format!("{}{}", "floor:   ", x.floor()));
    println!("{}", format!("{}{}", "ceil:    ", x.ceil()));
    println!("{}", format!("{}{}", "round:   ", x.round()));
    // A predicate returns a bool.
    println!("{}", format!("{}{}", "is_nan:  ", x.is_nan()));
    // The inferred type still drives coercion: abs() of a Long stays a Long.
    let size: i64 = n.abs();
    println!("{}", format!("{}{}", "size+1:  ", size + 1));
}
