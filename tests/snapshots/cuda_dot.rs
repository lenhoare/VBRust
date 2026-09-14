// Dot product on the GPU: each product is independent; adding them is Parallel Sum.

#[allow(dead_code)]
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


#[allow(dead_code, unused_mut, unused_variables, unused_assignments, unused_unsafe)]
struct __VbrCudaBuffer<T> {
    ptr: u64,
    len: usize,
    cols: usize,
    managed: bool,
    _t: std::marker::PhantomData<T>,
}

#[allow(dead_code)]
impl<T> __VbrCudaBuffer<T> {
    fn len(&self) -> usize {
        self.len
    }
    fn count(&self) -> usize {
        self.len
    }
    fn cols(&self) -> usize {
        self.cols
    }
}

impl<T> std::ops::Index<usize> for __VbrCudaBuffer<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        assert!(
            self.managed,
            "host index needs CUDA.Managed — Upload/Alloc stay on the device. CUDA.Download, or index inside Parallel For."
        );
        let n = if self.cols == 0 {
            self.len
        } else {
            self.len.saturating_mul(self.cols)
        };
        assert!(i < n, "CudaBuffer index out of bounds");
        unsafe { &*(self.ptr as *const T).add(i) }
    }
}
impl<T> std::ops::IndexMut<usize> for __VbrCudaBuffer<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        let n = if self.cols == 0 {
            self.len
        } else {
            self.len.saturating_mul(self.cols)
        };
        assert!(
            self.managed,
            "host index needs CUDA.Managed — Upload/Alloc stay on the device. CUDA.Download, or index inside Parallel For."
        );
        assert!(i < n, "CudaBuffer index out of bounds");
        unsafe { &mut *(self.ptr as *mut T).add(i) }
    }
}

impl<T> Drop for __VbrCudaBuffer<T> {
    fn drop(&mut self) {
        if self.ptr != 0 {
            let _ = __vbr_cuda_free(self.ptr);
            self.ptr = 0;
        }
    }
}

const __VBR_CUDA_NEEDED: &str = "CUDA needs an NVIDIA GPU and driver (libcuda / nvcuda.dll). There is no silent \
CPU copy — ordinary Vec Parallel For stays on the CPU. Install a driver, or keep this work on a Vec.";

const __VBR_NVRTC_NEEDED: &str = "The GPU is there, but compiling a Parallel For kernel needs the \
CUDA toolkit (libnvrtc / nvrtc64_*.dll). Install the toolkit, or keep this loop on a Vec.";

#[allow(dead_code)]
fn __vbr_cuda_upload<T: Copy>(xs: &[T]) -> Result<__VbrCudaBuffer<T>, String> {
    let bytes = std::mem::size_of_val(xs);
    let ptr = __vbr_cuda_alloc_bytes(bytes)?;
    if bytes > 0 {
        __vbr_cuda_copy_hto_d(ptr, xs.as_ptr() as *const u8, bytes)?;
    }
    Ok(__VbrCudaBuffer {
        ptr,
        len: xs.len(),
        cols: 0,
        managed: false,
        _t: std::marker::PhantomData,
    })
}

#[allow(dead_code)]
fn __vbr_cuda_upload_2d<T: Copy>(rows: &[Vec<T>]) -> Result<__VbrCudaBuffer<T>, String> {
    let ny = rows.len();
    let nx = rows.first().map(|r| r.len()).unwrap_or(0);
    for r in rows {
        if r.len() != nx {
            return Err(
                "CUDA.Upload of a 2-D list needs a rectangle — every row the same length.".into(),
            );
        }
    }
    let mut flat = Vec::with_capacity(ny.saturating_mul(nx));
    for r in rows {
        flat.extend_from_slice(r);
    }
    let bytes = std::mem::size_of_val(flat.as_slice());
    let ptr = __vbr_cuda_alloc_bytes(bytes)?;
    if bytes > 0 {
        __vbr_cuda_copy_hto_d(ptr, flat.as_ptr() as *const u8, bytes)?;
    }
    Ok(__VbrCudaBuffer {
        ptr,
        len: ny,
        cols: nx,
        managed: false,
        _t: std::marker::PhantomData,
    })
}

#[allow(dead_code)]
fn __vbr_cuda_alloc<T>(n: i64) -> Result<__VbrCudaBuffer<T>, String> {
    if n < 0 {
        return Err("CUDA.Alloc(n) needs a non-negative length.".into());
    }
    let n = n as usize;
    let bytes = n.saturating_mul(std::mem::size_of::<T>());
    let ptr = __vbr_cuda_alloc_bytes(bytes)?;
    if bytes > 0 {
        __vbr_cuda_memset(ptr, bytes)?;
    }
    Ok(__VbrCudaBuffer {
        ptr,
        len: n,
        cols: 0,
        managed: false,
        _t: std::marker::PhantomData,
    })
}

#[allow(dead_code)]
fn __vbr_cuda_alloc_2d<T>(rows: i64, cols: i64) -> Result<__VbrCudaBuffer<T>, String> {
    if rows < 0 || cols < 0 {
        return Err("CUDA.Alloc(rows, cols) needs non-negative sizes.".into());
    }
    let ny = rows as usize;
    let nx = cols as usize;
    let n = ny.saturating_mul(nx);
    let bytes = n.saturating_mul(std::mem::size_of::<T>());
    let ptr = __vbr_cuda_alloc_bytes(bytes)?;
    if bytes > 0 {
        __vbr_cuda_memset(ptr, bytes)?;
    }
    Ok(__VbrCudaBuffer {
        ptr,
        len: ny,
        cols: nx,
        managed: false,
        _t: std::marker::PhantomData,
    })
}

#[allow(dead_code)]
fn __vbr_cuda_managed<T>(n: i64) -> Result<__VbrCudaBuffer<T>, String> {
    if n < 0 {
        return Err("CUDA.Managed(n) needs a non-negative length.".into());
    }
    let n = n as usize;
    let bytes = n.saturating_mul(std::mem::size_of::<T>());
    let ptr = __vbr_cuda_alloc_managed_bytes(bytes)?;
    if bytes > 0 {
        __vbr_cuda_memset(ptr, bytes)?;
    }
    Ok(__VbrCudaBuffer {
        ptr,
        len: n,
        cols: 0,
        managed: true,
        _t: std::marker::PhantomData,
    })
}

#[allow(dead_code)]
fn __vbr_cuda_managed_2d<T>(rows: i64, cols: i64) -> Result<__VbrCudaBuffer<T>, String> {
    if rows < 0 || cols < 0 {
        return Err("CUDA.Managed(rows, cols) needs non-negative sizes.".into());
    }
    let ny = rows as usize;
    let nx = cols as usize;
    let n = ny.saturating_mul(nx);
    let bytes = n.saturating_mul(std::mem::size_of::<T>());
    let ptr = __vbr_cuda_alloc_managed_bytes(bytes)?;
    if bytes > 0 {
        __vbr_cuda_memset(ptr, bytes)?;
    }
    Ok(__VbrCudaBuffer {
        ptr,
        len: ny,
        cols: nx,
        managed: true,
        _t: std::marker::PhantomData,
    })
}

#[allow(dead_code)]
fn __vbr_cuda_prefetch<T>(buf: &__VbrCudaBuffer<T>, host: bool) -> Result<(), String> {
    if !buf.managed {
        return Err(
            "CUDA.Prefetch is for CUDA.Managed buffers — Upload/Alloc stay on the device.".into(),
        );
    }
    let n = if buf.cols == 0 {
        buf.len
    } else {
        buf.len.saturating_mul(buf.cols)
    };
    let bytes = n.saturating_mul(std::mem::size_of::<T>());
    if bytes == 0 || buf.ptr == 0 {
        return Ok(());
    }
    __vbr_cuda_prefetch_bytes(buf.ptr, bytes, host)
}

#[allow(dead_code)]
fn __vbr_cuda_sync() -> Result<(), String> {
    __vbr_cuda_ctx_sync()
}

#[allow(dead_code)]
fn __vbr_cuda_download<T: Copy + Default>(buf: &__VbrCudaBuffer<T>) -> Result<Vec<T>, String> {
    let n = if buf.cols == 0 {
        buf.len
    } else {
        buf.len.saturating_mul(buf.cols)
    };
    let mut out = vec![T::default(); n];
    let bytes = std::mem::size_of_val(out.as_slice());
    if bytes > 0 {
        __vbr_cuda_copy_d_to_h(out.as_mut_ptr() as *mut u8, buf.ptr, bytes)?;
    }
    Ok(out)
}

#[allow(dead_code)]
fn __vbr_cuda_download_2d<T: Copy + Default>(
    buf: &__VbrCudaBuffer<T>,
) -> Result<Vec<Vec<T>>, String> {
    let nx = buf.cols;
    let ny = buf.len;
    let n = ny.saturating_mul(nx);
    let mut flat = vec![T::default(); n];
    let bytes = std::mem::size_of_val(flat.as_slice());
    if bytes > 0 {
        __vbr_cuda_copy_d_to_h(flat.as_mut_ptr() as *mut u8, buf.ptr, bytes)?;
    }
    if nx == 0 {
        return Ok(vec![Vec::new(); ny]);
    }
    Ok(flat.chunks(nx).map(|r| r.to_vec()).collect())
}

#[allow(dead_code)]
fn __vbr_cuda_for(
    n: usize,
    src: &str,
    ptrs: &[u64],
    from: i64,
    step: i64,
) -> Result<(), String> {
    if n == 0 {
        return Ok(());
    }
    if n > u32::MAX as usize {
        return Err("CUDA Parallel For is too large for one launch.".into());
    }
    let fun = __vbr_cuda_compile(src)?;
    let mut slots: Vec<u64> = ptrs.to_vec();
    let mut from = from;
    let mut step = step;
    let mut n_i = n as i64;
    let mut args: Vec<*mut std::ffi::c_void> = slots
        .iter_mut()
        .map(|p| p as *mut u64 as *mut std::ffi::c_void)
        .collect();
    args.push(&mut from as *mut i64 as *mut std::ffi::c_void);
    args.push(&mut step as *mut i64 as *mut std::ffi::c_void);
    args.push(&mut n_i as *mut i64 as *mut std::ffi::c_void);
    let block: u32 = 256;
    let grid: u32 = ((n as u32) + block - 1) / block;
    __vbr_cuda_launch(fun, grid, 1, block, 1, &mut args)
}

#[allow(dead_code)]
fn __vbr_cuda_for_2d(
    ny: usize,
    nx: usize,
    src: &str,
    ptrs: &[u64],
    cols: &[u64],
    yfrom: i64,
    ystep: i64,
    xfrom: i64,
    xstep: i64,
) -> Result<(), String> {
    if ny == 0 || nx == 0 {
        return Ok(());
    }
    if ny > u32::MAX as usize || nx > u32::MAX as usize {
        return Err("CUDA Parallel For is too large for one launch.".into());
    }
    let fun = __vbr_cuda_compile(src)?;
    let mut slots: Vec<u64> = ptrs.to_vec();
    let mut col_slots: Vec<i64> = cols.iter().map(|c| *c as i64).collect();
    let mut yfrom = yfrom;
    let mut ystep = ystep;
    let mut ny_i = ny as i64;
    let mut xfrom = xfrom;
    let mut xstep = xstep;
    let mut nx_i = nx as i64;
    let mut args: Vec<*mut std::ffi::c_void> = Vec::new();
    for (p, c) in slots.iter_mut().zip(col_slots.iter_mut()) {
        args.push(p as *mut u64 as *mut std::ffi::c_void);
        args.push(c as *mut i64 as *mut std::ffi::c_void);
    }
    args.push(&mut yfrom as *mut i64 as *mut std::ffi::c_void);
    args.push(&mut ystep as *mut i64 as *mut std::ffi::c_void);
    args.push(&mut ny_i as *mut i64 as *mut std::ffi::c_void);
    args.push(&mut xfrom as *mut i64 as *mut std::ffi::c_void);
    args.push(&mut xstep as *mut i64 as *mut std::ffi::c_void);
    args.push(&mut nx_i as *mut i64 as *mut std::ffi::c_void);
    let bx: u32 = 16;
    let by: u32 = 16;
    let gx: u32 = ((nx as u32) + bx - 1) / bx;
    let gy: u32 = ((ny as u32) + by - 1) / by;
    __vbr_cuda_launch(fun, gx, gy, bx, by, &mut args)
}

#[allow(dead_code)]
fn __vbr_cuda_sum_src(cty: &str) -> String {
    let zero = match cty {
        "float" => "0.0f",
        "double" => "0.0",
        _ => "0",
    };
    format!(
        "extern \"C\" __global__ void k({cty}* __in, {cty}* __out, long long __n) {{\n    __shared__ {cty} __s[256];\n    long long __i = (long long)blockIdx.x * (long long)blockDim.x + (long long)threadIdx.x;\n    {cty} __v = (__i < __n) ? __in[__i] : ({cty}){zero};\n    __s[threadIdx.x] = __v;\n    __syncthreads();\n    for (int __stride = 128; __stride > 0; __stride >>= 1) {{\n        if ((int)threadIdx.x < __stride) {{\n            __s[threadIdx.x] = __s[threadIdx.x] + __s[threadIdx.x + __stride];\n        }}\n        __syncthreads();\n    }}\n    if (threadIdx.x == 0) __out[blockIdx.x] = __s[0];\n}}\n"
    )
}

#[allow(dead_code)]
fn __vbr_cuda_sum<T: Copy + Default>(buf: &__VbrCudaBuffer<T>, cty: &str) -> Result<T, String> {
    if buf.cols != 0 {
        return Err("Parallel Sum needs a 1-D CudaBuffer.".into());
    }
    let n = buf.len;
    if n == 0 {
        return Ok(T::default());
    }
    if n > u32::MAX as usize {
        return Err("CUDA Parallel Sum is too large for one launch.".into());
    }
    let sz = std::mem::size_of::<T>();
    if n == 1 {
        let mut out = T::default();
        __vbr_cuda_copy_d_to_h(&mut out as *mut T as *mut u8, buf.ptr, sz)?;
        return Ok(out);
    }
    let fun = __vbr_cuda_compile(&__vbr_cuda_sum_src(cty))?;
    let scratch_n = (n + 255) / 256;
    let s1 = __vbr_cuda_alloc_bytes(scratch_n.saturating_mul(sz))?;
    let s2 = __vbr_cuda_alloc_bytes(scratch_n.saturating_mul(sz))?;
    let result = (|| {
        let mut in_ptr = buf.ptr;
        let mut n = n;
        let mut dest = s1;
        let mut alt = s2;
        loop {
            let block: u32 = 256;
            let grid: u32 = ((n as u32) + block - 1) / block;
            let mut in_slot = in_ptr;
            let mut out_slot = dest;
            let mut n_i = n as i64;
            let mut args: Vec<*mut std::ffi::c_void> = vec![
                &mut in_slot as *mut u64 as *mut std::ffi::c_void,
                &mut out_slot as *mut u64 as *mut std::ffi::c_void,
                &mut n_i as *mut i64 as *mut std::ffi::c_void,
            ];
            __vbr_cuda_launch(fun, grid, 1, block, 1, &mut args)?;
            if grid == 1 {
                let mut out = T::default();
                __vbr_cuda_copy_d_to_h(&mut out as *mut T as *mut u8, dest, sz)?;
                return Ok(out);
            }
            n = grid as usize;
            in_ptr = dest;
            dest = alt;
            alt = in_ptr;
        }
    })();
    let _ = __vbr_cuda_free(s1);
    let _ = __vbr_cuda_free(s2);
    result
}

#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_alloc_bytes(_: usize) -> Result<u64, String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_free(_: u64) -> Result<(), String> {
    Ok(())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_copy_hto_d(_: u64, _: *const u8, _: usize) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_copy_d_to_h(_: *mut u8, _: u64, _: usize) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_memset(_: u64, _: usize) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_compile(_: &str) -> Result<u64, String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_launch(
    _: u64,
    _: u32,
    _: u32,
    _: u32,
    _: u32,
    _: &mut [*mut std::ffi::c_void],
) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_alloc_managed_bytes(_: usize) -> Result<u64, String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_prefetch_bytes(_: u64, _: usize, _: bool) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(any(unix, windows)))]
fn __vbr_cuda_ctx_sync() -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}

#[cfg(any(unix, windows))]
mod __vbr_cuda_drv {
    #![allow(dead_code, unused_unsafe)]
    use std::collections::HashMap;
    use std::ffi::{CString, c_char, c_int, c_uint, c_void};
    use std::sync::{Mutex, OnceLock};

    #[cfg(unix)]
    const RTLD_NOW: c_int = 2;

    #[cfg(unix)]
    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    #[cfg(windows)]
    extern "system" {
        fn LoadLibraryA(name: *const c_char) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    struct Api {
        cu_init: unsafe extern "C" fn(c_uint) -> c_int,
        cu_device_get: unsafe extern "C" fn(*mut c_int, c_int) -> c_int,
        cu_ctx_create: unsafe extern "C" fn(*mut usize, c_uint, c_int) -> c_int,
        cu_mem_alloc: unsafe extern "C" fn(*mut u64, usize) -> c_int,
        cu_mem_alloc_managed: unsafe extern "C" fn(*mut u64, usize, c_uint) -> c_int,
        cu_mem_free: unsafe extern "C" fn(u64) -> c_int,
        cu_memcpy_htod: unsafe extern "C" fn(u64, *const c_void, usize) -> c_int,
        cu_memcpy_dtoh: unsafe extern "C" fn(*mut c_void, u64, usize) -> c_int,
        cu_memset: unsafe extern "C" fn(u64, c_uint, usize) -> c_int,
        cu_mem_prefetch: unsafe extern "C" fn(u64, usize, c_int, usize) -> c_int,
        cu_module_load_data: unsafe extern "C" fn(*mut usize, *const c_void) -> c_int,
        cu_module_get_function: unsafe extern "C" fn(*mut usize, usize, *const c_char) -> c_int,
        cu_launch: unsafe extern "C" fn(
            usize,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            usize,
            *mut *mut c_void,
            *mut *mut c_void,
        ) -> c_int,
        cu_sync: unsafe extern "C" fn() -> c_int,
        nvrtc_create: unsafe extern "C" fn(
            *mut usize,
            *const c_char,
            *const c_char,
            c_int,
            *const *const c_char,
            *const *const c_char,
        ) -> c_int,
        nvrtc_compile: unsafe extern "C" fn(usize, c_int, *const *const c_char) -> c_int,
        nvrtc_ptx_size: unsafe extern "C" fn(usize, *mut usize) -> c_int,
        nvrtc_ptx: unsafe extern "C" fn(usize, *mut c_char) -> c_int,
        nvrtc_log_size: unsafe extern "C" fn(usize, *mut usize) -> c_int,
        nvrtc_log: unsafe extern "C" fn(usize, *mut c_char) -> c_int,
        nvrtc_destroy: unsafe extern "C" fn(usize) -> c_int,
        _cuda: *mut c_void,
        _nvrtc: *mut c_void,
        device: c_int,
    }

    unsafe impl Send for Api {}
    unsafe impl Sync for Api {}

    fn try_load(name: &str) -> Option<*mut c_void> {
        let c = CString::new(name).ok()?;
        #[cfg(unix)]
        let h = unsafe { dlopen(c.as_ptr(), RTLD_NOW) };
        #[cfg(windows)]
        let h = unsafe { LoadLibraryA(c.as_ptr()) };
        if h.is_null() {
            None
        } else {
            Some(h)
        }
    }

    fn load_lib(names: &[&str]) -> Result<*mut c_void, ()> {
        for n in names {
            if let Some(h) = try_load(n) {
                return Ok(h);
            }
        }
        #[cfg(windows)]
        if let Ok(root) = std::env::var("CUDA_PATH") {
            let bin = format!("{}\\bin", root.trim_end_matches(['\\', '/']));
            for n in names {
                if let Some(h) = try_load(&format!("{bin}\\{n}")) {
                    return Ok(h);
                }
            }
        }
        Err(())
    }

    unsafe fn sym(h: *mut c_void, names: &[&str]) -> Result<*mut c_void, String> {
        for n in names {
            let c = CString::new(*n).unwrap();
            #[cfg(unix)]
            let p = dlsym(h, c.as_ptr());
            #[cfg(windows)]
            let p = GetProcAddress(h, c.as_ptr());
            if !p.is_null() {
                return Ok(p);
            }
        }
        Err(format!("CUDA symbol {} missing", names[0]))
    }

    fn api() -> Result<&'static Api, String> {
        static API: OnceLock<Result<Api, String>> = OnceLock::new();
        match API.get_or_init(|| unsafe { load_api() }) {
            Ok(a) => Ok(a),
            Err(e) => Err(e.clone()),
        }
    }

    unsafe fn load_api() -> Result<Api, String> {
        let cuda = load_lib(&[
            #[cfg(unix)]
            "libcuda.so.1",
            #[cfg(unix)]
            "libcuda.so",
            #[cfg(windows)]
            "nvcuda.dll",
        ])
        .map_err(|_| super::__VBR_CUDA_NEEDED.to_string())?;
        let nvrtc = load_lib(&[
            #[cfg(unix)]
            "libnvrtc.so.13",
            #[cfg(unix)]
            "libnvrtc.so.12",
            #[cfg(unix)]
            "libnvrtc.so.11",
            #[cfg(unix)]
            "libnvrtc.so",
            #[cfg(windows)]
            "nvrtc64_130_0.dll",
            #[cfg(windows)]
            "nvrtc64_128_0.dll",
            #[cfg(windows)]
            "nvrtc64_126_0.dll",
            #[cfg(windows)]
            "nvrtc64_124_0.dll",
            #[cfg(windows)]
            "nvrtc64_120_0.dll",
            #[cfg(windows)]
            "nvrtc64_118_0.dll",
            #[cfg(windows)]
            "nvrtc64_112_0.dll",
            #[cfg(windows)]
            "nvrtc64_110_0.dll",
            #[cfg(windows)]
            "nvrtc64_12.dll",
            #[cfg(windows)]
            "nvrtc.dll",
        ])
        .map_err(|_| super::__VBR_NVRTC_NEEDED.to_string())?;
        let cu_init = std::mem::transmute(sym(cuda, &["cuInit"])?);
        let cu_device_get = std::mem::transmute(sym(cuda, &["cuDeviceGet"])?);
        let cu_ctx_create =
            std::mem::transmute(sym(cuda, &["cuCtxCreate_v2", "cuCtxCreate"])?);
        let cu_mem_alloc = std::mem::transmute(sym(cuda, &["cuMemAlloc_v2", "cuMemAlloc"])?);
        let cu_mem_alloc_managed = std::mem::transmute(sym(cuda, &["cuMemAllocManaged"])?);
        let cu_mem_free = std::mem::transmute(sym(cuda, &["cuMemFree_v2", "cuMemFree"])?);
        let cu_memcpy_htod =
            std::mem::transmute(sym(cuda, &["cuMemcpyHtoD_v2", "cuMemcpyHtoD"])?);
        let cu_memcpy_dtoh =
            std::mem::transmute(sym(cuda, &["cuMemcpyDtoH_v2", "cuMemcpyDtoH"])?);
        let cu_memset = std::mem::transmute(sym(cuda, &["cuMemsetD8_v2", "cuMemsetD8"])?);
        let cu_mem_prefetch = std::mem::transmute(sym(cuda, &["cuMemPrefetchAsync"])?);
        let cu_module_load_data = std::mem::transmute(sym(cuda, &["cuModuleLoadData"])?);
        let cu_module_get_function = std::mem::transmute(sym(cuda, &["cuModuleGetFunction"])?);
        let cu_launch = std::mem::transmute(sym(cuda, &["cuLaunchKernel"])?);
        let cu_sync = std::mem::transmute(sym(cuda, &["cuCtxSynchronize"])?);
        let nvrtc_create = std::mem::transmute(sym(nvrtc, &["nvrtcCreateProgram"])?);
        let nvrtc_compile = std::mem::transmute(sym(nvrtc, &["nvrtcCompileProgram"])?);
        let nvrtc_ptx_size = std::mem::transmute(sym(nvrtc, &["nvrtcGetPTXSize"])?);
        let nvrtc_ptx = std::mem::transmute(sym(nvrtc, &["nvrtcGetPTX"])?);
        let nvrtc_log_size = std::mem::transmute(sym(nvrtc, &["nvrtcGetProgramLogSize"])?);
        let nvrtc_log = std::mem::transmute(sym(nvrtc, &["nvrtcGetProgramLog"])?);
        let nvrtc_destroy = std::mem::transmute(sym(nvrtc, &["nvrtcDestroyProgram"])?);
        let mut api = Api {
            cu_init,
            cu_device_get,
            cu_ctx_create,
            cu_mem_alloc,
            cu_mem_alloc_managed,
            cu_mem_free,
            cu_memcpy_htod,
            cu_memcpy_dtoh,
            cu_memset,
            cu_mem_prefetch,
            cu_module_load_data,
            cu_module_get_function,
            cu_launch,
            cu_sync,
            nvrtc_create,
            nvrtc_compile,
            nvrtc_ptx_size,
            nvrtc_ptx,
            nvrtc_log_size,
            nvrtc_log,
            nvrtc_destroy,
            _cuda: cuda,
            _nvrtc: nvrtc,
            device: 0,
        };
        check((api.cu_init)(0), "cuInit")?;
        let mut dev: c_int = 0;
        check((api.cu_device_get)(&mut dev, 0), "cuDeviceGet")?;
        let mut ctx: usize = 0;
        check((api.cu_ctx_create)(&mut ctx, 0, dev), "cuCtxCreate")?;
        let _ = ctx;
        api.device = dev;
        Ok(api)
    }

    fn check(st: c_int, what: &str) -> Result<(), String> {
        if st == 0 {
            Ok(())
        } else {
            Err(format!("{what} failed (CUDA error {st})"))
        }
    }

    fn nvrtc_log(api: &Api, prog: usize) -> String {
        let mut n = 0usize;
        if unsafe { (api.nvrtc_log_size)(prog, &mut n) } != 0 || n == 0 {
            return String::new();
        }
        let mut buf = vec![0u8; n];
        if unsafe { (api.nvrtc_log)(prog, buf.as_mut_ptr() as *mut c_char) } != 0 {
            return String::new();
        }
        String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string()
    }

    static KERNELS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();

    pub fn alloc_bytes(n: usize) -> Result<u64, String> {
        if n == 0 {
            return Ok(0);
        }
        let api = api()?;
        let mut ptr = 0u64;
        check(unsafe { (api.cu_mem_alloc)(&mut ptr, n) }, "cuMemAlloc")?;
        Ok(ptr)
    }

    pub fn free(ptr: u64) -> Result<(), String> {
        if ptr == 0 {
            return Ok(());
        }
        let api = api()?;
        check(unsafe { (api.cu_mem_free)(ptr) }, "cuMemFree")
    }

    pub fn copy_hto_d(dst: u64, src: *const u8, n: usize) -> Result<(), String> {
        let api = api()?;
        check(
            unsafe { (api.cu_memcpy_htod)(dst, src as *const c_void, n) },
            "cuMemcpyHtoD",
        )
    }

    pub fn copy_d_to_h(dst: *mut u8, src: u64, n: usize) -> Result<(), String> {
        let api = api()?;
        check(
            unsafe { (api.cu_memcpy_dtoh)(dst as *mut c_void, src, n) },
            "cuMemcpyDtoH",
        )
    }

    pub fn memset(ptr: u64, n: usize) -> Result<(), String> {
        let api = api()?;
        check(unsafe { (api.cu_memset)(ptr, 0, n) }, "cuMemsetD8")
    }

    pub fn alloc_managed(n: usize) -> Result<u64, String> {
        if n == 0 {
            return Ok(0);
        }
        let api = api()?;
        let mut ptr = 0u64;
        check(
            unsafe { (api.cu_mem_alloc_managed)(&mut ptr, n, 1) },
            "cuMemAllocManaged",
        )?;
        Ok(ptr)
    }

    pub fn prefetch(ptr: u64, n: usize, host: bool) -> Result<(), String> {
        if ptr == 0 || n == 0 {
            return Ok(());
        }
        let api = api()?;
        let dest: c_int = if host { -1 } else { api.device };
        check(
            unsafe { (api.cu_mem_prefetch)(ptr, n, dest, 0) },
            "cuMemPrefetchAsync",
        )
    }

    pub fn sync() -> Result<(), String> {
        let api = api()?;
        check(unsafe { (api.cu_sync)() }, "cuCtxSynchronize")
    }

    pub fn compile(src: &str) -> Result<usize, String> {
        let api = api()?;
        let cache = KERNELS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(fun) = cache.lock().unwrap().get(src).copied() {
            return Ok(fun);
        }
        let mut prog: usize = 0;
        let src_c = CString::new(src).map_err(|_| "kernel source")?;
        let name = CString::new("k.cu").unwrap();
        check(
            unsafe {
                (api.nvrtc_create)(&mut prog, src_c.as_ptr(), name.as_ptr(), 0, std::ptr::null(), std::ptr::null())
            },
            "nvrtcCreateProgram",
        )?;
        let st = unsafe { (api.nvrtc_compile)(prog, 0, std::ptr::null()) };
        if st != 0 {
            let log = nvrtc_log(api, prog);
            unsafe { (api.nvrtc_destroy)(prog) };
            return Err(if log.trim().is_empty() {
                format!("nvrtcCompileProgram failed (error {st})")
            } else {
                format!("CUDA kernel compile failed:\n{log}")
            });
        }
        let mut ptx_n = 0usize;
        check(unsafe { (api.nvrtc_ptx_size)(prog, &mut ptx_n) }, "nvrtcGetPTXSize")?;
        let mut ptx = vec![0u8; ptx_n];
        check(
            unsafe { (api.nvrtc_ptx)(prog, ptx.as_mut_ptr() as *mut c_char) },
            "nvrtcGetPTX",
        )?;
        unsafe { (api.nvrtc_destroy)(prog) };
        let mut module: usize = 0;
        check(
            unsafe { (api.cu_module_load_data)(&mut module, ptx.as_ptr() as *const c_void) },
            "cuModuleLoadData",
        )?;
        let mut fun: usize = 0;
        let kname = CString::new("k").unwrap();
        check(
            unsafe { (api.cu_module_get_function)(&mut fun, module, kname.as_ptr()) },
            "cuModuleGetFunction",
        )?;
        cache.lock().unwrap().insert(src.to_string(), fun);
        Ok(fun)
    }

    pub fn launch(
        fun: usize,
        grid_x: u32,
        grid_y: u32,
        block_x: u32,
        block_y: u32,
        args: &mut [*mut c_void],
    ) -> Result<(), String> {
        let api = api()?;
        check(
            unsafe {
                (api.cu_launch)(
                    fun,
                    grid_x,
                    grid_y,
                    1,
                    block_x,
                    block_y,
                    1,
                    0,
                    0,
                    args.as_mut_ptr(),
                    std::ptr::null_mut(),
                )
            },
            "cuLaunchKernel",
        )?;
        check(unsafe { (api.cu_sync)() }, "cuCtxSynchronize")
    }
}

#[cfg(any(unix, windows))]
fn __vbr_cuda_alloc_bytes(n: usize) -> Result<u64, String> {
    __vbr_cuda_drv::alloc_bytes(n)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_free(ptr: u64) -> Result<(), String> {
    __vbr_cuda_drv::free(ptr)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_copy_hto_d(dst: u64, src: *const u8, n: usize) -> Result<(), String> {
    __vbr_cuda_drv::copy_hto_d(dst, src, n)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_copy_d_to_h(dst: *mut u8, src: u64, n: usize) -> Result<(), String> {
    __vbr_cuda_drv::copy_d_to_h(dst, src, n)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_memset(ptr: u64, n: usize) -> Result<(), String> {
    __vbr_cuda_drv::memset(ptr, n)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_alloc_managed_bytes(n: usize) -> Result<u64, String> {
    __vbr_cuda_drv::alloc_managed(n)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_prefetch_bytes(ptr: u64, n: usize, host: bool) -> Result<(), String> {
    __vbr_cuda_drv::prefetch(ptr, n, host)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_ctx_sync() -> Result<(), String> {
    __vbr_cuda_drv::sync()
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_compile(src: &str) -> Result<u64, String> {
    __vbr_cuda_drv::compile(src).map(|p| p as u64)
}
#[cfg(any(unix, windows))]
fn __vbr_cuda_launch(
    fun: u64,
    grid_x: u32,
    grid_y: u32,
    block_x: u32,
    block_y: u32,
    args: &mut [*mut std::ffi::c_void],
) -> Result<(), String> {
    __vbr_cuda_drv::launch(fun as usize, grid_x, grid_y, block_x, block_y, args)
}

fn vbr_main() -> Result<(), String> {
    let xs: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    let ys: Vec<f32> = vec![5.0, 6.0, 7.0, 8.0];
    let n: i64 = xs.len() as i64;
    let a: __VbrCudaBuffer<f32> = __vbr_cuda_upload((xs).as_slice())?;
    let b: __VbrCudaBuffer<f32> = __vbr_cuda_upload((ys).as_slice())?;
    let prod: __VbrCudaBuffer<f32> = __vbr_cuda_alloc(n)?;
    {
        let __from = 0;
        let __to = n - 1;
        let __n: usize = if __to >= __from { ((__to - __from) as usize).saturating_add(1) } else { 0 };
        __vbr_cuda_for(__n, "extern \"C\" __global__ void k(float* a, float* b, float* prod, long long __from, long long __step, long long __n) {\n    long long __k = (long long)blockIdx.x * (long long)blockDim.x + (long long)threadIdx.x;\n    if (__k >= __n) return;\n    long long i = __from + __k * __step;\n    prod[i] = (a[i] * b[i]);\n}\n", &[a.ptr, b.ptr, prod.ptr], __from as i64, 1)?;
    }
    let total: f32 = __vbr_cuda_sum(&prod, "float")?;
    println!("{}", total);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
