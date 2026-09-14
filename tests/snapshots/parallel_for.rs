// Parallel For — independent iterations, run on CPU threads.

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

fn vbr_main() -> Result<(), String> {
    let input: Vec<i64> = vec![1, 2, 3, 4];
    let mut output: Vec<i64> = vec![0, 0, 0, 0];
    {
        let __from = 0;
        let __to = (input.len() as i32) - 1;
        let __n: usize = if __to >= __from { ((__to - __from) as usize).saturating_add(1) } else { 0 };
        let __p_output = output.as_mut_ptr() as usize;
        __vbr_parallel_for(__n, &|__k| {
            #[allow(unused_variables)]
            let i = __from + (__k as i32);
            { unsafe { *__vbr_at(__p_output, (i) as usize) = input[(i) as usize] * input[(i) as usize] } };
        });
    }
    for n in &output {
        println!("{}", *n);
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
