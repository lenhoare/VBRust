// Vec<T> — a growable list

fn main() {
    let mut nums: Vec<i32> = Vec::new();
    nums.push(10);
    nums.push(20);
    nums.push(30);
    println!("{}", format!("{}{}", "count = ", nums.len()));
    let mut total: i32 = 0;
    for n in &nums {
        total = total + *n;
    }
    println!("{}", format!("{}{}", "total = ", total));
}
