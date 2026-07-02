// Iterators — filter, map, sum, any, count, collect

fn main() {
    let mut nums: Vec<i64> = Vec::new();
    nums.push(1);
    nums.push(2);
    nums.push(3);
    nums.push(4);
    nums.push(5);
    let big: Vec<i64> = nums.iter().copied().filter(|&x| x > 2).collect();
    let doubled: Vec<i64> = nums.iter().copied().map(|x| x * 2).collect();
    let total: i64 = nums.iter().copied().sum();
    let has_big: bool = nums.iter().copied().any(|x| x > 4);
    println!("count:   {}", nums.iter().copied().count());
    println!("total:   {}", total);
    println!("has big: {}", has_big);
    for n in &big {
        println!("big:     {}", *n);
    }
    for n in &doubled {
        println!("doubled: {}", *n);
    }
}
