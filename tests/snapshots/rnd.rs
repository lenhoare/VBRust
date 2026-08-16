// Rnd() is a Double in [0, 1). A die is Int(Rnd() * 6) + 1.
// The printed line is a range check so the snapshot stays deterministic.

fn rnd() -> f64 {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = Cell::new({
            #[cfg(target_arch = "wasm32")]
            { 0xA0761D6478BD642Fu64 }
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0xA0761D6478BD642F)
            }
        });
    }
    STATE.with(|s| {
        let mut z = s.get().wrapping_add(0x9E3779B97F4A7C15);
        s.set(z);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
    })
}

fn vbr_main() -> Result<(), String> {
    let r: f64 = rnd();
    if r >= 0.0 && r < 1.0 {
        println!("ok");
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
