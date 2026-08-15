// VB's two ways to turn text into a number, and how they differ.
// 
// Val   — the *lenient* one. Always a Double, ignores surrounding spaces,
// and returns 0 for text that isn't a number. It never fails, so
// there is nothing to handle.
// CDbl / CLng / CInt — the *strict* conversions. On text that isn't a
// number they fail. The error propagates automatically, or you
// intercept it with Handle. Use these when bad input is an error
// you want to catch, not silently turn into 0.

fn priceof(txt: &str) -> Result<f64, String> {
    // A bad CDbl fails this function — the caller Handle's it, or Main exits.
    Ok(txt.trim().parse::<f64>().map_err(|e| e.to_string())?)
}

fn vbr_main() -> Result<(), String> {
    // Lenient: 0 on nonsense, spaces ignored, always a Double.
    println!("{}", "3.14".trim().parse::<f64>().unwrap_or(0.0));
    println!("{}", "  42  ".trim().parse::<f64>().unwrap_or(0.0));
    println!("{}", "nonsense".trim().parse::<f64>().unwrap_or(0.0));
    // A Double flows into a Long with Bust's automatic numeric cast.
    let count: i64 = "100".trim().parse::<f64>().unwrap_or(0.0) as i64;
    println!("{}", count);
    // Strict: intercept failure with Handle.
    #[allow(unused_mut)]
    let mut v: i64;
    v = match "77".trim().parse::<i64>().map_err(|e| e.to_string()) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(e) => {
            println!("not a number: {}", e);
            return Ok(());
        }
    };
    println!("parsed {}", v);
    #[allow(unused_mut)]
    let mut p: f64;
    p = match priceof("9.99") {
        Ok(__vbr_ok) => __vbr_ok,
        Err(e) => {
            println!("{}", e);
            return Ok(());
        }
    };
    println!("price is {}", p);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
