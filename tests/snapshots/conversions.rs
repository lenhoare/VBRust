// VB's two ways to turn text into a number, and how they differ.
// 
// Val   — the *lenient* one. Always a Double, ignores surrounding spaces,
// and returns 0 for text that isn't a number. It never fails, so
// there is nothing to handle.
// CDbl / CLng / CInt — the *strict* conversions. On text that isn't a
// number they fail, handing back a Result you deal with using `?`
// (propagate) or `Match` (branch). Use these when bad input is an
// error you want to catch, not silently turn into 0.

fn priceof(txt: &str) -> Result<f64, String> {
    // `?` bails out of this function with the error string on bad input.
    let price: f64 = txt.trim().parse::<f64>().map_err(|e| e.to_string())?;
    Ok(price)
}

fn main() {
    // Lenient: 0 on nonsense, spaces ignored, always a Double.
    println!("{}", "3.14".trim().parse::<f64>().unwrap_or(0.0));
    println!("{}", "  42  ".trim().parse::<f64>().unwrap_or(0.0));
    println!("{}", "nonsense".trim().parse::<f64>().unwrap_or(0.0));
    // A Double flows into a Long with VBR's automatic numeric cast.
    let count: i64 = "100".trim().parse::<f64>().unwrap_or(0.0) as i64;
    println!("{}", count);
    // Strict: branch on success or failure.
    match "77".trim().parse::<i64>().map_err(|e| e.to_string()) {
        Ok ( v ) => {
            println!("parsed {}", v);
        }
        Err ( e ) => {
            println!("not a number: {}", e);
        }
    }
    match priceof("9.99") {
        Ok ( p ) => {
            println!("price is {}", p);
        }
        Err ( e ) => {
            println!("{}", e);
        }
    }
}
