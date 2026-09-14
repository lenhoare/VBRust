// Parallel reduction: each round writes dest[i] and reads a previous array
// at 2*i. dest starts as `[0; n]` — one fill, no Push loop, no Resize keyword.

fn __vbr_parallel_for(n: usize, f: &(dyn Fn(usize) + Sync)) {
    if n == 0 {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    {
        for k in 0..n {
            f(k);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
            .clamp(1, n);
        let chunk = (n + threads - 1) / threads;
        std::thread::scope(|scope| {
            for t in 0..threads {
                let start = t * chunk;
                if start >= n {
                    break;
                }
                let end = (start + chunk).min(n);
                scope.spawn(move || {
                    for k in start..end {
                        f(k);
                    }
                });
            }
        });
    }
}

#[allow(dead_code)]
#[inline(always)]
unsafe fn __vbr_at<T>(p: usize, i: usize) -> *mut T {
    (p as *mut T).add(i)
}

fn halve(src: &Vec<i64>) -> Result<Vec<i64>, String> {
    let n: i64 = ((src.len() as f64) / (2 as f64)) as i64;
    let mut dest: Vec<i64> = vec![0; ((n) as i64).max(0) as usize];
    {
        let __from = 0;
        let __to = n - 1;
        let __n: usize = if __to >= __from { ((__to - __from) as usize).saturating_add(1) } else { 0 };
        let __p_dest = dest.as_mut_ptr() as usize;
        __vbr_parallel_for(__n, &|__k| {
            #[allow(unused_variables)]
            let i = __from + (__k as i64);
            { unsafe { *__vbr_at(__p_dest, (i) as usize) = src[(2 * i) as usize] + src[(2 * i + 1) as usize] } };
        });
    }
    Ok(dest)
}

fn vbr_main() -> Result<(), String> {
    let mut src: Vec<i64> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    while (src.len() as i32) > 1 {
        src = halve(&src)?;
    }
    println!("{}", src[0]);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
