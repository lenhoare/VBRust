// DateTime from the standard library — parse a fixed moment, then read, format
// and shift it. (Uses Parse, not Now, so the output is deterministic.)

use vbr_stdlib::{DateTime};

fn main() {
    let d: DateTime = DateTime::parse("2026-07-24 09:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    println!("year:  {}", d.year());
    println!("month: {}", d.month());
    println!("day:   {}", d.day());
    println!("iso:   {}", d.format("%Y-%m-%d"));
    let later: DateTime = d.add_days(10);
    println!("in 10 days: {}", later.format("%Y-%m-%d"));
    let soon: DateTime = d.add_hours(5);
    println!("in 5 hours: {}", soon.format("%H:%M"));
}
