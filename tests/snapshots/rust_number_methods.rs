// Rust number methods pass through too — same deal as strings. Each returns the
// receiver's own type, so casts and concatenation still line up.

fn main() {
    let n: i64 = -42;
    // Integer methods.
    println!("abs:     {}", n.abs());
    println!("max(0):  {}", n.max(0));
    println!("clamp:   {}", n.clamp(-10, 10));
    let base: i64 = 2;
    println!("2^10:    {}", base.pow((10) as u32));
    // Float methods.
    let x: f64 = 7.3;
    println!("sqrt:    {}", x.sqrt());
    println!("floor:   {}", x.floor());
    println!("ceil:    {}", x.ceil());
    println!("round:   {}", x.round());
    // A predicate returns a bool.
    println!("is_nan:  {}", x.is_nan());
    // The inferred type still drives coercion: abs() of a Long stays a Long.
    let size: i64 = n.abs();
    println!("size+1:  {}", size + 1);
}
