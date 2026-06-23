// Select Case → match

fn main() {
    let score: i64 = 75;
    match score {
        100 => {
            println!("{}", "perfect");
        }
        90..=99 => {
            println!("{}", "excellent");
        }
        70..=89 => {
            println!("{}", "good");
        }
        0 | 1 | 2 => {
            println!("{}", "very low");
        }
        _ => {
            println!("{}", "somewhere in between");
        }
    }
}
