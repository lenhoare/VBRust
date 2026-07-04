// More iterator links — take, skip, rev — and the Option-returning consumers
// (max, position) that pair with Match.

fn main() {
    let hi: i64 = 8;
    let mut nums: Vec<i64> = Vec::new();
    for i in 1..=hi {
        nums.push(i);
    }
    let firstthree: Vec<i64> = nums.iter().copied().take(3).collect();
    let lasttwo: Vec<i64> = nums.iter().copied().rev().take(2).collect();
    let tail: Vec<i64> = nums.iter().copied().skip(5).collect();
    for n in &firstthree {
        println!("take: {}", *n);
    }
    for n in &lasttwo {
        println!("rev:  {}", *n);
    }
    for n in &tail {
        println!("skip: {}", *n);
    }
    match nums.iter().copied().max() {
        Some ( top ) => {
            println!("max:  {}", top);
        }
        None => {
            println!("empty");
        }
    }
    match nums.iter().copied().position(|x| x > 6) {
        Some ( idx ) => {
            println!("pos:  {}", idx);
        }
        None => {
            println!("none over 6");
        }
    }
}
