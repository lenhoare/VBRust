// Iterators — filter, map, sum, any, count, collect

fn main() {
    let mut nums: Vec<i32> = Vec::new();
    nums.push(1);
    nums.push(2);
    nums.push(3);
    nums.push(4);
    nums.push(5);
    let big: Vec<i32> = nums.iter().copied().filter(|&x| x > 2).collect();
    let doubled: Vec<i32> = nums.iter().copied().map(|x| x * 2).collect();
    let total: i32 = nums.iter().copied().sum();
    let has_big: bool = nums.iter().copied().any(|x| x > 4);
    println!("{}", format!("{}{}", "count:   ", nums.iter().copied().count()));
    println!("{}", format!("{}{}", "total:   ", total));
    println!("{}", format!("{}{}", "has big: ", has_big));
    for n in &big {
        println!("{}", format!("{}{}", "big:     ", *n));
    }
    for n in &doubled {
        println!("{}", format!("{}{}", "doubled: ", *n));
    }
}
