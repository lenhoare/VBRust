// Numeric/borrow coercions the transpiler inserts so real programs compile:
// integer ^ → .pow, Vec index/compare → usize casts, For Each over a borrowed
// Vec param, and .Clone() on a &str parameter → .to_string().

fn square(n: i64) -> i64 {
    n.pow((2) as u32)
}

fn sum_and_first(nums: &Vec<i64>) -> i64 {
    let mut total: i64 = 0;
    for x in &*nums {
        total += *x;
    }
    let i: i64 = 0;
    if i < (nums.len() as i64) {
        total += nums[(i) as usize];
    }
    total
}

fn dup(s: &str) -> String {
    s.to_string()
}

fn main() {
    let v: Vec<i64> = { vec![10, 20, 30] };
    println!("square(3)  = {}", square(3));
    println!("sum+first  = {}", sum_and_first(&v));
    println!("dup        = {}", dup("hi"));
}
