// Tuples — literals, indexing, multiple return values, destructuring, patterns

fn main() {
    let pair: (i64, i64) = (3, 7);
    println!("first = {}", pair.0);
    println!("sum   = {}", pair.0 + pair.1);
    let (lo, hi) = min_max(10, 4);
    println!("min = {}, max = {}", lo, hi);
    match classify(0, 5) {
        ( 0 , y ) => {
            println!("on the y-axis at {}", y);
        }
        ( x , 0 ) => {
            println!("on the x-axis at {}", x);
        }
        _ => {
            println!("off both axes");
        }
    }
}

fn min_max(a: i64, b: i64) -> (i64, i64) {
    if a < b {
        return (a, b);
    }
    (b, a)
}

fn classify(x: i64, y: i64) -> (i64, i64) {
    (x, y)
}
