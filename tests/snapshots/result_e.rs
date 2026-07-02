// Result<T, E> with a real, typed error enum — including a message-carrying
// variant. Build errors with Err(MathError.…); read them back by matching. `?`
// works when the error types line up.

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
enum MathError {
    DivByZero,
    Custom(String),
}

fn safediv(a: i32, b: i32) -> Result<i32, MathError> {
    if b == 0 {
        return Err(MathError::DivByZero);
    }
    if b < 0 {
        return Err(MathError::Custom("negative divisor".to_string()));
    }
    Ok(a / b)
}

fn doublediv(a: i32, b: i32) -> Result<i32, MathError> {
    let q: i32 = safediv(a, b)?;
    Ok(q * 2)
}

fn main() {
    match doublediv(10, 2) {
        Ok ( v ) => {
            println!("ok: {}", v);
        }
        Err ( MathError :: DivByZero ) => {
            println!("div by zero");
        }
        Err ( MathError :: Custom ( msg ) ) => {
            println!("error: {}", msg);
        }
    }
    match doublediv(10, 0) {
        Ok ( v ) => {
            println!("ok: {}", v);
        }
        Err ( _ ) => {
            println!("failed");
        }
    }
    match doublediv(10, -2) {
        Ok ( v ) => {
            println!("ok: {}", v);
        }
        Err ( MathError :: DivByZero ) => {
            println!("div by zero");
        }
        Err ( MathError :: Custom ( msg ) ) => {
            println!("error: {}", msg);
        }
    }
}
