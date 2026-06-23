// Do loops, Exit and Continue

fn main() {
    let mut i: i64 = 1;
    while i <= 3 {
        println!("{}", format!("{}{}", "while ", i));
        i = i + 1;
    }
    let mut j: i64 = 10;
    while !(j == 0) {
        j = j - 2;
    }
    println!("{}", format!("{}{}", "j ended at ", j));
    let mut n: i64 = 0;
    loop {
        n = n + 1;
        if !(n < 3) {
            break;
        }
    }
    println!("{}", format!("{}{}", "n = ", n));
    for k in 1..=6 {
        if k == 4 {
            break;
        }
        if k == 2 {
            continue;
        }
        println!("{}", format!("{}{}", "k = ", k));
    }
}
