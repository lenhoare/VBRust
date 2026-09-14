// Nested Parallel For — a 2-D index space. One CPU launch over ny * nx.

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
    let src: Vec<Vec<i64>> = vec![vec![1, 2, 3], vec![4, 5, 6]];
    let mut dest: Vec<Vec<i64>> = vec![vec![0; 3]; 2];
    {
        let __from_y = 0;
        let __to_y = 1;
        let __n_y: usize = if __to_y >= __from_y { ((__to_y - __from_y) as usize).saturating_add(1) } else { 0 };
        let __from_x = 0;
        let __to_x = 2;
        let __n_x: usize = if __to_x >= __from_x { ((__to_x - __from_x) as usize).saturating_add(1) } else { 0 };
        let __n: usize = __n_y.saturating_mul(__n_x);
        let __p_dest = dest.as_mut_ptr() as usize;
        __vbr_parallel_for(__n, &|__k| {
            #[allow(unused_variables)]
            let __ky = if __n_x == 0 { 0 } else { __k / __n_x };
            let __kx = if __n_x == 0 { 0 } else { __k % __n_x };
            let y = __from_y + (__ky as i32);
            let x = __from_x + (__kx as i32);
            { unsafe { (&mut (*__vbr_at::<Vec<_>>(__p_dest, (y) as usize)))[(x) as usize] = src[(y) as usize].clone()[(x) as usize] * 2 } };
        });
    }
    println!("{}", dest[0].clone()[0]);
    println!("{}", dest[1].clone()[2]);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
