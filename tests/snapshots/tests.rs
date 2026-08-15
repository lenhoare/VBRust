// A `Test "description" … End Test` block is an executable specification: run it
// with `vbr test`, which reports `✓ / ✗` by description and shows the operand
// values on a failure. Each `Assert <expr>` lowers to a Rust assertion — `=` and
// `<>` become `assert_eq!`/`assert_ne!` (so you see `left` vs `right`), anything
// else an `assert!`. Tests live under `#[cfg(test)]`, so `vbr run`/`build` ignore
// them; only `vbr test` builds and runs them. In a project, gather a module's
// tests in a `<module>.test.vbr` file beside it.

fn fizzbuzz(n: i64) -> Result<String, String> {
    if n % 15 == 0 {
        return Ok("fizzbuzz".to_string());
    }
    if n % 3 == 0 {
        return Ok("fizz".to_string());
    }
    if n % 5 == 0 {
        return Ok("buzz".to_string());
    }
    Ok(n.to_string())
}

fn vbr_main() -> Result<(), String> {
    let mut i: i64 = 1;
    while i <= 15 {
        println!("{}", fizzbuzz(i)?);
        i = i + 1;
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod vbr_tests {
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn multiples_of_three_are_fizz() -> Result<(), String> {
        assert_eq!(fizzbuzz(9)?, "fizz");
        Ok(())
    }
    #[test]
    fn multiples_of_five_are_buzz() -> Result<(), String> {
        assert_eq!(fizzbuzz(10)?, "buzz");
        Ok(())
    }
    #[test]
    fn multiples_of_fifteen_are_fizzbuzz() -> Result<(), String> {
        assert_eq!(fizzbuzz(30)?, "fizzbuzz");
        Ok(())
    }
    #[test]
    fn an_ordinary_number_is_its_own_text() -> Result<(), String> {
        assert_eq!(fizzbuzz(7)?, "7");
        assert_ne!(fizzbuzz(7)?, "fizz");
        Ok(())
    }
}
