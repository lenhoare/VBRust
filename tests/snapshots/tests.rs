// A `Test "description" … End Test` block is an executable specification: run it
// with `vbr test`, which reports `✓ / ✗` by description and shows the operand
// values on a failure. Each `Assert <expr>` lowers to a Rust assertion — `=` and
// `<>` become `assert_eq!`/`assert_ne!` (so you see `left` vs `right`), anything
// else an `assert!`. Tests live under `#[cfg(test)]`, so `vbr run`/`build` ignore
// them; only `vbr test` builds and runs them. In a project, gather a module's
// tests in a `<module>.test.vbr` file beside it.

fn fizzbuzz(n: i64) -> String {
    if n % 15 == 0 {
        return "fizzbuzz".to_string();
    }
    if n % 3 == 0 {
        return "fizz".to_string();
    }
    if n % 5 == 0 {
        return "buzz".to_string();
    }
    n.to_string()
}

fn main() {
    let mut i: i64 = 1;
    while i <= 15 {
        println!("{}", fizzbuzz(i));
        i = i + 1;
    }
}

#[cfg(test)]
mod vbr_tests {
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn multiples_of_three_are_fizz() {
        assert_eq!(fizzbuzz(9), "fizz");
    }
    #[test]
    fn multiples_of_five_are_buzz() {
        assert_eq!(fizzbuzz(10), "buzz");
    }
    #[test]
    fn multiples_of_fifteen_are_fizzbuzz() {
        assert_eq!(fizzbuzz(30), "fizzbuzz");
    }
    #[test]
    fn an_ordinary_number_is_its_own_text() {
        assert_eq!(fizzbuzz(7), "7");
        assert_ne!(fizzbuzz(7), "fizz");
    }
}
