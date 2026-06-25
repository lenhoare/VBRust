// Tuples — literals, indexing, multiple return values, destructuring, patterns

fn main() {
    let pair: (i64, i64) = (3, 7);
    println!("{}", format!("{}{}", "first = ", pair.0));
    println!("{}", format!("{}{}", "sum   = ", pair.0 + pair.1));
    let (lo, hi) = min_max(10, 4);
    println!("{}", format!("{}{}", format!("{}{}", format!("{}{}", "min = ", lo), ", max = "), hi));
    match classify(0, 5) {
        ( 0 , y ) => {
            println!("{}", format!("{}{}", "on the y-axis at ", y));
        }
        ( x , 0 ) => {
            println!("{}", format!("{}{}", "on the x-axis at ", x));
        }
        _ => {
            println!("{}", "off both axes");
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
