// Parallel Sum — first-class reduction. CPU threads, no atomics.

fn __vbr_parallel_sum<T>(xs: &[T]) -> T
where
    T: Copy + Default + std::ops::Add<Output = T> + Send + Sync,
{
    let n = xs.len();
    if n == 0 {
        return T::default();
    }
    #[cfg(target_arch = "wasm32")]
    {
        return xs.iter().copied().fold(T::default(), |a, b| a + b);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
            .clamp(1, n);
        let chunk = (n + threads - 1) / threads;
        let mut parts = vec![T::default(); threads];
        std::thread::scope(|scope| {
            for (t, slot) in parts.iter_mut().enumerate() {
                let start = t * chunk;
                if start >= n {
                    break;
                }
                let end = (start + chunk).min(n);
                let slice = &xs[start..end];
                scope.spawn(move || {
                    *slot = slice.iter().copied().fold(T::default(), |a, b| a + b);
                });
            }
        });
        parts.into_iter().fold(T::default(), |a, b| a + b)
    }
}

fn vbr_main() -> Result<(), String> {
    let xs: Vec<i64> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let total: i64 = __vbr_parallel_sum((xs).as_slice());
    println!("{}", total);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
