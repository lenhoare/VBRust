// VB6 habits, translated. This example locks in four fixes found building a
// Game of Life:
// - a ByRef collection forwarded into another ByRef call is already the
// `&mut` the callee wants — it passes bare, and Rust reborrows;
// - `Dim i As Long` ahead of `For i = …` would be a dead `let` (the For makes
// its own binding), so VBR drops it;
// - an owned-String needle (`Mid` returns a String) borrows for
// contains/starts_with — a bare String isn't a Pattern;
// - `Dim x = lines[i]` clones the element out of a Vec instead of moving it.

fn setcell(grid: &mut Vec<i64>, idx: i64, v: i64) {
    grid[(idx) as usize] = v;
}

fn placeblinker(grid: &mut Vec<i64>, at: i64) {
    setcell(grid, at, 1);
    setcell(grid, at + 1, 1);
    setcell(grid, at + 2, 1);
}

fn digitcount(s: &str) -> i64 {
    let digits: String = "0123456789".to_string();
    let mut total: i64 = 0;
    for i in 1..=s.len() as i32 {
        let ch: String = s.chars().skip(((i) - 1) as usize).take(1).collect::<String>().to_string();
        if digits.contains(&ch) {
            total = total + 1;
        }
    }
    total
}

fn main() {
    let mut grid: Vec<i64> = vec![0, 0, 0, 0, 0];
    placeblinker(&mut grid, 1);
    let mut live: i64 = 0;
    for cell in &grid {
        live = live + *cell;
    }
    println!("live cells: {}", live);
    println!("digits in b3s23: {}", digitcount("b3s23"));
    let rules: Vec<String> = vec!["B3/S23".to_string(), "B36/S23".to_string()];
    let rule: String = rules[1].clone();
    if rule.starts_with("B36") {
        println!("highlife: {}", rule);
    }
}
