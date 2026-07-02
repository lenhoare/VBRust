// Compound assignment — +=, -=, *=, /= (a modern convenience over `a = a + 1`).

fn main() {
    let mut n: i64 = 10;
    n += 5;
    n -= 3;
    n *= 2;
    n /= 4;
    println!("n = {}", n);
}
