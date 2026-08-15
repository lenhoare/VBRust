// Arrays — fixed size, 2D, and safe access with .get()

fn vbr_main() -> Result<(), String> {
    let mut scores: [i64; 5] = [0; 5];
    scores[0] = 90;
    scores[1] = 85;
    scores[2] = 78;
    println!("scores[1] = {}", scores[1]);
    let mut grid: [[i64; 3]; 2] = [[0; 3]; 2];
    grid[1][2] = 42;
    println!("grid[1][2] = {}", grid[1][2]);
    // .get() returns an Option, so out-of-bounds is handled, not a crash
    match scores.get(0).copied() {
        Some ( v ) => {
            println!("first score = {}", v);
        }
        None => {
            println!("no first score");
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
