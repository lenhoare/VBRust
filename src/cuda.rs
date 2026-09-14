//! `CudaBuffer<T>` and `CUDA.Upload` / `Alloc` / `Managed` / `Download`.
//!
//! CUDA names **where** work runs. `Parallel For` over those buffers is the GPU
//! claim; ordinary `Vec` Parallel For stays on CPU threads. There is no silent
//! host copy of a `Vec` onto the device. `CUDA.Managed` is still a `CudaBuffer`
//! — host index is allowed; movement is `Prefetch` / `PrefetchHost` / `Sync`.

use std::cell::RefCell;
use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::rust_name;

thread_local! {
    static SRC_FNS: RefCell<Vec<Function>> = const { RefCell::new(Vec::new()) };
    static TWO_D: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Makes same-file `Function`s visible while resolving/emitting a CUDA kernel.
pub struct SrcFns;
pub struct SrcFnsGuard;

impl SrcFns {
    pub fn install(fns: &[Function]) -> SrcFnsGuard {
        SRC_FNS.with(|s| *s.borrow_mut() = fns.to_vec());
        SrcFnsGuard
    }
}

impl Drop for SrcFnsGuard {
    fn drop(&mut self) {
        SRC_FNS.with(|s| s.borrow_mut().clear());
    }
}

/// Why Python and C refuse CUDA rather than faking a host `Vec`.
pub const RUST_ONLY: &str = "`CudaBuffer` and `CUDA.Upload` are Rust-only. Python and C have no \
device backend, and we won't emit a host array that pretends otherwise. Run it with `vbr run`.";

/// Runtime for device buffers and kernel launch. Emitted only when the program
/// uses `CudaBuffer` / `CUDA.*`. Dynamically loads the driver and NVRTC
/// (`libcuda`/`libnvrtc` on Unix, `nvcuda.dll`/`nvrtc64_*.dll` on Windows) —
/// no CUDA toolkit link, so CPU-only machines still `rustc` the program.
pub const CUDA_HELPER: &str = r#"
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
"#;

pub fn is_cuda_ns(name: &str) -> bool {
    name.eq_ignore_ascii_case("cuda")
}

pub fn is_cuda_method(method: &str) -> bool {
    matches!(
        method.to_ascii_lowercase().replace('_', "").as_str(),
        "upload" | "upload2d" | "alloc" | "alloc2d" | "managed" | "managed2d"
            | "download" | "download2d" | "prefetch" | "prefetchhost" | "sync"
    )
}

/// Element type and whether this `CudaBuffer` is a 2-D grid (`CudaBuffer<CudaBuffer<T>>`).
pub fn cuda_leaf(ty: &DeclType) -> Option<(Type, bool)> {
    match ty {
        DeclType::CudaBuffer(inner) => match inner.as_ref() {
            DeclType::Plain(t) if t.is_number() => Some((*t, false)),
            DeclType::CudaBuffer(elem) => match elem.as_ref() {
                DeclType::Plain(t) if t.is_number() => Some((*t, true)),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

pub fn cuda_c_type(t: Type) -> &'static str {
    match t {
        Type::Integer => "int",
        Type::Long | Type::LongLong => "long long",
        Type::Byte => "unsigned char",
        Type::Single => "float",
        Type::Double => "double",
        Type::Boolean => "int",
        Type::Text | Type::Usize => "long long",
    }
}

/// True when the program names `CudaBuffer` or `CUDA.*`.
pub fn program_uses_cuda(program: &Program) -> bool {
    let any = |stmts: &[Stmt]| stmts.iter().any(stmt_uses_cuda);
    program.functions.iter().any(|f| {
        f.params.iter().any(|p| ty_uses_cuda(&p.ty))
            || f.ret.as_ref().is_some_and(ty_uses_cuda)
            || any(&f.body)
    }) || program.tests.iter().any(|t| any(&t.body))
        || program.structs.iter().any(|s| s.fields.iter().any(|f| ty_uses_cuda(&f.ty)))
        || program.windows.iter().any(|w| {
            w.events.iter().any(|e| any(&e.body)) || w.subs.iter().any(|s| any(&s.body))
        })
        || program.sketches.iter().any(|s| {
            any(&s.draw)
                || s.events.iter().any(|e| any(&e.body))
                || s.subs.iter().any(|sub| any(&sub.body))
        })
        || program.screens.iter().any(|s| {
            s.events.iter().any(|e| any(&e.body)) || s.subs.iter().any(|sub| any(&sub.body))
        })
        || program.pages.iter().any(|p| {
            p.events.iter().any(|e| any(&e.body)) || p.subs.iter().any(|s| any(&s.body))
        })
        || program.canvases.iter().any(|c| any(&c.body))
        || program.godot_nodes.iter().any(|n| {
            n.events.iter().any(|e| any(&e.body)) || n.handlers.iter().any(|h| any(&h.body))
        })
}

fn ty_uses_cuda(ty: &DeclType) -> bool {
    match ty {
        DeclType::CudaBuffer(_) => true,
        DeclType::Vec(t) | DeclType::Option(t) => ty_uses_cuda(t),
        DeclType::Map(a, b) | DeclType::Result(a, b) => ty_uses_cuda(a) || ty_uses_cuda(b),
        DeclType::Tuple(ts) => ts.iter().any(ty_uses_cuda),
        _ => false,
    }
}

fn stmt_uses_cuda(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Dim { ty, init, .. } => {
            ty_uses_cuda(ty) || init.as_ref().is_some_and(expr_uses_cuda)
        }
        Stmt::Set { value: e, .. }
        | Stmt::DestructureDim { value: e, .. }
        | Stmt::Return(Some(e))
        | Stmt::Print(e)
        | Stmt::Log(_, e)
        | Stmt::Expr(e)
        | Stmt::RaiseError(e)
        | Stmt::Assert(e) => expr_uses_cuda(e),
        Stmt::Assign { target, value, .. } => expr_uses_cuda(target) || expr_uses_cuda(value),
        Stmt::If { branches, else_body } => {
            branches
                .iter()
                .any(|(c, b)| expr_uses_cuda(c) || b.iter().any(stmt_uses_cuda))
                || else_body.as_ref().is_some_and(|b| b.iter().any(stmt_uses_cuda))
        }
        Stmt::For { from, to, step, body, .. } => {
            expr_uses_cuda(from)
                || expr_uses_cuda(to)
                || step.as_ref().is_some_and(expr_uses_cuda)
                || body.iter().any(stmt_uses_cuda)
        }
        Stmt::ForEach { iter, body, .. } => {
            expr_uses_cuda(iter) || body.iter().any(stmt_uses_cuda)
        }
        Stmt::DoLoop { cond, body } => {
            let in_cond = match cond {
                Some(
                    DoCond::PreWhile(c)
                    | DoCond::PreUntil(c)
                    | DoCond::PostWhile(c)
                    | DoCond::PostUntil(c),
                ) => expr_uses_cuda(c),
                None => false,
            };
            in_cond || body.iter().any(stmt_uses_cuda)
        }
        Stmt::Match { scrutinee, arms, .. } => {
            expr_uses_cuda(scrutinee)
                || arms.iter().any(|a| {
                    a.guard.as_ref().is_some_and(expr_uses_cuda)
                        || a.body.iter().any(stmt_uses_cuda)
                })
        }
        Stmt::HandleErr { call, body, target, .. } => {
            expr_uses_cuda(call)
                || target.as_ref().is_some_and(expr_uses_cuda)
                || body.iter().any(stmt_uses_cuda)
        }
        Stmt::GpuInto { body, .. } => body.iter().any(stmt_uses_cuda),
        _ => false,
    }
}

fn expr_uses_cuda(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::MethodCall { recv, method, args } => {
            (matches!(&recv.kind, ExprKind::Ident(n) if is_cuda_ns(n)) && is_cuda_method(method))
                || expr_uses_cuda(recv)
                || args.iter().any(expr_uses_cuda)
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Index(lhs, rhs)
        | ExprKind::ListRepeat { value: lhs, count: rhs } => {
            expr_uses_cuda(lhs) || expr_uses_cuda(rhs)
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            args.iter().any(expr_uses_cuda)
        }
        ExprKind::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_uses_cuda(v)),
        ExprKind::Field(inner, _)
        | ExprKind::Deref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Ref(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Not(inner)
        | ExprKind::Await(inner)
        | ExprKind::ParallelSum(inner, _)
        | ExprKind::TupleIndex(inner, _)
        | ExprKind::Closure { body: inner, .. } => expr_uses_cuda(inner),
        _ => false,
    }
}

/// Names of collections this block indexes (`a[i]`, `b[i]`).
pub fn indexed_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut out = HashSet::new();
    for s in stmts {
        walk_stmt_index(s, &mut out);
    }
    out
}

fn walk_stmt_index(s: &Stmt, out: &mut HashSet<String>) {
    match s {
        Stmt::Dim { init: Some(e), .. }
        | Stmt::Set { value: e, .. }
        | Stmt::DestructureDim { value: e, .. }
        | Stmt::Return(Some(e))
        | Stmt::Print(e)
        | Stmt::Log(_, e)
        | Stmt::Expr(e)
        | Stmt::RaiseError(e)
        | Stmt::Assert(e) => walk_expr_index(e, out),
        Stmt::Assign { target, value, .. } => {
            walk_expr_index(target, out);
            walk_expr_index(value, out);
        }
        Stmt::If { branches, else_body } => {
            for (c, b) in branches {
                walk_expr_index(c, out);
                for s in b {
                    walk_stmt_index(s, out);
                }
            }
            if let Some(b) = else_body {
                for s in b {
                    walk_stmt_index(s, out);
                }
            }
        }
        Stmt::For { from, to, step, body, .. } => {
            walk_expr_index(from, out);
            walk_expr_index(to, out);
            if let Some(st) = step {
                walk_expr_index(st, out);
            }
            for s in body {
                walk_stmt_index(s, out);
            }
        }
        Stmt::ForEach { iter, body, .. } => {
            walk_expr_index(iter, out);
            for s in body {
                walk_stmt_index(s, out);
            }
        }
        Stmt::DoLoop { cond, body } => {
            match cond {
                Some(
                    DoCond::PreWhile(c)
                    | DoCond::PreUntil(c)
                    | DoCond::PostWhile(c)
                    | DoCond::PostUntil(c),
                ) => walk_expr_index(c, out),
                None => {}
            }
            for s in body {
                walk_stmt_index(s, out);
            }
        }
        Stmt::Match { scrutinee, arms, .. } => {
            walk_expr_index(scrutinee, out);
            for a in arms {
                if let Some(g) = &a.guard {
                    walk_expr_index(g, out);
                }
                for s in &a.body {
                    walk_stmt_index(s, out);
                }
            }
        }
        Stmt::HandleErr { call, body, target, .. } => {
            walk_expr_index(call, out);
            if let Some(t) = target {
                walk_expr_index(t, out);
            }
            for s in body {
                walk_stmt_index(s, out);
            }
        }
        Stmt::GpuInto { body, .. } => {
            for s in body {
                walk_stmt_index(s, out);
            }
        }
        _ => {}
    }
}

fn walk_expr_index(e: &Expr, out: &mut HashSet<String>) {
    match &e.kind {
        ExprKind::Index(inner, idx) => {
            if let ExprKind::Ident(n) = &inner.kind {
                out.insert(n.clone());
            } else {
                walk_expr_index(inner, out);
            }
            walk_expr_index(idx, out);
        }
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::ListRepeat { value: lhs, count: rhs } => {
            walk_expr_index(lhs, out);
            walk_expr_index(rhs, out);
        }
        ExprKind::MethodCall { recv, args, .. } => {
            walk_expr_index(recv, out);
            for a in args {
                walk_expr_index(a, out);
            }
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            for a in args {
                walk_expr_index(a, out);
            }
        }
        ExprKind::StructLit { fields, .. } => {
            for (_, v) in fields {
                walk_expr_index(v, out);
            }
        }
        ExprKind::Field(inner, _)
        | ExprKind::Deref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Ref(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Not(inner)
        | ExprKind::Await(inner)
        | ExprKind::ParallelSum(inner, _)
        | ExprKind::TupleIndex(inner, _)
        | ExprKind::Closure { body: inner, .. } => walk_expr_index(inner, out),
        _ => {}
    }
}

/// Extra idents a device body reads that aren't the loop var, a local, or a buffer.
pub fn stray_idents(stmts: &[Stmt], allow: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    for s in stmts {
        walk_stmt_idents(s, allow, &mut out);
    }
    out
}

fn walk_stmt_idents(s: &Stmt, allow: &HashSet<String>, out: &mut HashSet<String>) {
    match s {
        Stmt::Dim { init: Some(e), .. }
        | Stmt::Set { value: e, .. }
        | Stmt::Return(Some(e))
        | Stmt::Print(e)
        | Stmt::Log(_, e)
        | Stmt::Expr(e)
        | Stmt::RaiseError(e)
        | Stmt::Assert(e) => walk_expr_idents(e, allow, out),
        Stmt::Assign { target, value, .. } => {
            walk_expr_idents(target, allow, out);
            walk_expr_idents(value, allow, out);
        }
        Stmt::If { branches, else_body } => {
            for (c, b) in branches {
                walk_expr_idents(c, allow, out);
                for s in b {
                    walk_stmt_idents(s, allow, out);
                }
            }
            if let Some(b) = else_body {
                for s in b {
                    walk_stmt_idents(s, allow, out);
                }
            }
        }
        Stmt::For { from, to, step, body, var, .. } => {
            walk_expr_idents(from, allow, out);
            walk_expr_idents(to, allow, out);
            if let Some(st) = step {
                walk_expr_idents(st, allow, out);
            }
            let mut inner = allow.clone();
            inner.insert(var.to_ascii_lowercase());
            for s in body {
                walk_stmt_idents(s, &inner, out);
            }
        }
        _ => {}
    }
}

fn walk_expr_idents(e: &Expr, allow: &HashSet<String>, out: &mut HashSet<String>) {
    match &e.kind {
        ExprKind::Ident(n) => {
            if !allow.contains(&n.to_ascii_lowercase()) && !is_cuda_ns(n) {
                out.insert(n.clone());
            }
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Index(lhs, rhs)
        | ExprKind::ListRepeat { value: lhs, count: rhs } => {
            walk_expr_idents(lhs, allow, out);
            walk_expr_idents(rhs, allow, out);
        }
        ExprKind::MethodCall { recv, args, .. } => {
            walk_expr_idents(recv, allow, out);
            for a in args {
                walk_expr_idents(a, allow, out);
            }
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            for a in args {
                walk_expr_idents(a, allow, out);
            }
        }
        ExprKind::Field(inner, _)
        | ExprKind::Deref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Ref(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Not(inner)
        | ExprKind::Await(inner)
        | ExprKind::ParallelSum(inner, _)
        | ExprKind::TupleIndex(inner, _)
        | ExprKind::Closure { body: inner, .. } => walk_expr_idents(inner, allow, out),
        _ => {}
    }
}

/// CUDA Parallel For body: assignments, `Dim` locals, and device-safe calls.
pub fn check_device_body(stmts: &[Stmt], line: usize, diags: &mut Diagnostics) -> bool {
    for s in stmts {
        match s {
            Stmt::LineMark(_) | Stmt::Comment(_) => {}
            Stmt::Assign { .. } => {}
            Stmt::Dim { .. } => {}
            Stmt::If { .. } => {
                diags.error(
                    line,
                    "Put the `If` in a numeric `Function` and call it from the loop. \
                     A CUDA `Parallel For` body is assignments (and `Dim` locals) this slice.",
                );
                return false;
            }
            Stmt::For { parallel: true, body, .. } => {
                if !check_device_body(body, line, diags) {
                    return false;
                }
            }
            Stmt::For { .. } => {
                diags.error(
                    line,
                    "A sequential `For` inside CUDA `Parallel For` isn't lowered \
                     yet. Keep the body as `buf[i] = …` assignments (and `Dim` locals).",
                );
                return false;
            }
            other => {
                diags.error(
                    line,
                    format!(
                        "CUDA `Parallel For` body is assignments, `Dim` locals, and \
                         device-safe calls this slice (`{}` isn't).",
                        stmt_kind(other)
                    ),
                );
                return false;
            }
        }
        if !check_calls_in_stmt(s, line, diags) {
            return false;
        }
        if stmt_has_host_expr(s) {
            diags.error(
                line,
                "CUDA `Parallel For` body is arithmetic on `buf[i]` — no method \
                 calls, no host functions. Compute on the device with `a[i] * 2.0`, \
                 or call a numeric `Function`.",
            );
            return false;
        }
    }
    true
}

fn check_calls_in_stmt(s: &Stmt, line: usize, diags: &mut Diagnostics) -> bool {
    let mut ok = true;
    walk_stmt_calls(s, &mut |name| {
        if !call_ok(name, line, diags) {
            ok = false;
        }
    });
    ok
}

fn walk_stmt_calls(s: &Stmt, f: &mut impl FnMut(&str)) {
    match s {
        Stmt::Assign { target, value, .. } => {
            walk_expr_calls(target, f);
            walk_expr_calls(value, f);
        }
        Stmt::Dim { init: Some(e), .. } | Stmt::Return(Some(e)) => walk_expr_calls(e, f),
        Stmt::If { branches, else_body } => {
            for (c, b) in branches {
                walk_expr_calls(c, f);
                for s in b {
                    walk_stmt_calls(s, f);
                }
            }
            if let Some(b) = else_body {
                for s in b {
                    walk_stmt_calls(s, f);
                }
            }
        }
        _ => {}
    }
}

fn walk_expr_calls(e: &Expr, f: &mut impl FnMut(&str)) {
    match &e.kind {
        ExprKind::Call { name, args } => {
            f(name);
            for a in args {
                walk_expr_calls(a, f);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index(lhs, rhs) => {
            walk_expr_calls(lhs, f);
            walk_expr_calls(rhs, f);
        }
        ExprKind::Not(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Deref(inner) => walk_expr_calls(inner, f),
        ExprKind::MethodCall { recv, args, .. } => {
            walk_expr_calls(recv, f);
            for a in args {
                walk_expr_calls(a, f);
            }
        }
        _ => {}
    }
}

fn call_ok(name: &str, line: usize, diags: &mut Diagnostics) -> bool {
    if is_device_math(name) {
        return true;
    }
    if name.eq_ignore_ascii_case("rnd") {
        diags.error(
            line,
            "`Rnd` is host-side. A CUDA `Parallel For` can call a numeric `Function` \
             (`Sin`, `Sqr`, …, or your own ByVal number-in / number-out helper).",
        );
        return false;
    }
    match device_fn_problem(name, &mut HashSet::new()) {
        None => true,
        Some(msg) => {
            diags.error(line, msg);
            false
        }
    }
}

fn is_device_math(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "sqr" | "abs" | "int" | "round" | "sin" | "cos" | "tan" | "atn" | "log" | "exp" | "iif"
    )
}

fn is_device_scalar(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(t) if t.is_number() || *t == Type::Boolean)
}

fn find_fn(name: &str) -> Option<Function> {
    let key = rust_name(name);
    SRC_FNS.with(|s| {
        s.borrow()
            .iter()
            .find(|f| f.receiver.is_none() && rust_name(&f.name) == key)
            .cloned()
    })
}

/// `None` if `name` is a device-safe same-file function.
fn device_fn_problem(name: &str, visiting: &mut HashSet<String>) -> Option<String> {
    let key = rust_name(name);
    if !visiting.insert(key.clone()) {
        return None;
    }
    let Some(f) = find_fn(name) else {
        return Some(format!(
            "CUDA `Parallel For` can call a numeric `Function` this slice — `{name}` \
             isn't one. Write `Function {name}(ByVal x As Single) As Single` (numbers \
             in, a number out), or keep the arithmetic in the loop."
        ));
    };
    if f.gpu {
        return Some(format!(
            "`{name}` is a `Gpu Function` (a Draw shader helper), not a CUDA device \
             function. Write an ordinary `Function` with ByVal numbers."
        ));
    }
    if f.params.iter().any(|p| p.mode == ParamMode::ByRef) {
        return Some(format!(
            "`{name}` takes `ByRef` — a CUDA helper is ByVal numbers (the kernel \
             already holds `buf[i]` as a scalar)."
        ));
    }
    if f.params.iter().any(|p| !is_device_scalar(&p.ty)) {
        return Some(format!(
            "`{name}` isn't device-safe — parameters must be numbers (or Boolean), \
             not a `Vec` / `String` / struct. Pass `a[i]`, not the buffer."
        ));
    }
    match &f.ret {
        Some(ty) if is_device_scalar(ty) => {}
        _ => {
            return Some(format!(
                "`{name}` needs a numeric return to run on the GPU (`As Single` / \
                 `As Long` / …). A `Sub` stays on the host."
            ));
        }
    }
    let mut allow = HashSet::new();
    for p in &f.params {
        allow.insert(p.name.to_ascii_lowercase());
    }
    crate::parallel::collect_locals(&f.body, &mut allow);
    if let Some(host) = stray_idents(&f.body, &allow).iter().next() {
        return Some(format!(
            "`{name}` reads `{host}`, which isn't a parameter or local — a CUDA \
             helper only sees the numbers you pass in."
        ));
    }
    if let Some(why) = device_fn_body_problem(&f.body) {
        return Some(format!(
            "`{name}` isn't device-safe ({why}). A CUDA helper is `If` / `Return` / \
             arithmetic on its parameters — no `Debug.Print`, no `FileSystem`, no `Vec`."
        ));
    }
    let mut nested_msg = None;
    walk_stmts_calls(&f.body, &mut |callee| {
        if nested_msg.is_some() || is_device_math(callee) {
            return;
        }
        if let Some(msg) = device_fn_problem(callee, visiting) {
            nested_msg = Some(msg);
        }
    });
    nested_msg
}

fn walk_stmts_calls(stmts: &[Stmt], f: &mut impl FnMut(&str)) {
    for s in stmts {
        walk_stmt_calls(s, f);
    }
}

/// True when some `Parallel For` in the program calls `name` (device kernels
/// inline the helper as CUDA C, so the Rust `fn` can look unused).
pub fn used_from_parallel(name: &str) -> bool {
    let key = rust_name(name);
    SRC_FNS.with(|s| {
        s.borrow().iter().any(|f| parallel_calls(&f.body, &key))
    })
}

fn parallel_calls(stmts: &[Stmt], key: &str) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::For { parallel: true, body, .. } => {
            let mut hit = false;
            walk_stmts_calls(body, &mut |n| {
                if rust_name(n) == key {
                    hit = true;
                }
            });
            hit || parallel_calls(body, key)
        }
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| parallel_calls(b, key))
                || else_body.as_ref().is_some_and(|b| parallel_calls(b, key))
        }
        Stmt::For { body, .. } | Stmt::DoLoop { body, .. } | Stmt::ForEach { body, .. } => {
            parallel_calls(body, key)
        }
        _ => false,
    })
}

fn device_fn_body_problem(stmts: &[Stmt]) -> Option<&'static str> {
    for s in stmts {
        match s {
            Stmt::LineMark(_) | Stmt::Comment(_) | Stmt::Assign { .. } | Stmt::Dim { .. } => {}
            Stmt::Return(_) => {}
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    if let Some(w) = device_fn_body_problem(b) {
                        return Some(w);
                    }
                }
                if let Some(b) = else_body {
                    if let Some(w) = device_fn_body_problem(b) {
                        return Some(w);
                    }
                }
            }
            other => return Some(stmt_kind(other)),
        }
        if stmt_has_host_expr(s) {
            return Some("a method call or host expression");
        }
    }
    None
}

fn stmt_has_host_expr(s: &Stmt) -> bool {
    match s {
        Stmt::Assign { target, value, .. } => expr_has_host(target) || expr_has_host(value),
        Stmt::Dim { init: Some(e), .. } | Stmt::Return(Some(e)) => expr_has_host(e),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(c, b)| {
                expr_has_host(c) || b.iter().any(stmt_has_host_expr)
            }) || else_body
                .as_ref()
                .is_some_and(|b| b.iter().any(stmt_has_host_expr))
        }
        _ => false,
    }
}

fn expr_has_host(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::MethodCall { .. } | ExprKind::ParallelSum(..) | ExprKind::Str(_) => true,
        ExprKind::Call { args, .. } => args.iter().any(expr_has_host),
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index(lhs, rhs) => {
            expr_has_host(lhs) || expr_has_host(rhs)
        }
        ExprKind::Not(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Deref(inner) => expr_has_host(inner),
        _ => false,
    }
}

fn stmt_kind(s: &Stmt) -> &'static str {
    match s {
        Stmt::If { .. } => "If",
        Stmt::Print(_) => "Debug.Print",
        Stmt::Return(_) => "Return",
        Stmt::Match { .. } => "Match",
        Stmt::ForEach { .. } => "For Each",
        Stmt::DoLoop { .. } => "Do",
        Stmt::Expr(_) => "a call",
        _ => "that statement",
    }
}

fn cuda_fn_ident(name: &str) -> String {
    format!(
        "__vbr_d_{}",
        rust_name(name).trim_start_matches("r#").replace('#', "")
    )
}

fn collect_helpers(stmts: &[Stmt]) -> Vec<Function> {
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    collect_helpers_from(stmts, &mut seen, &mut order);
    order
}

fn collect_helpers_from(stmts: &[Stmt], seen: &mut HashSet<String>, order: &mut Vec<Function>) {
    walk_stmts_calls(stmts, &mut |name| {
        if is_device_math(name) {
            return;
        }
        let Some(f) = find_fn(name) else {
            return;
        };
        if f.gpu {
            return;
        }
        let key = rust_name(&f.name);
        if !seen.insert(key) {
            return;
        }
        collect_helpers_from(&f.body, seen, order);
        order.push(f);
    });
}

fn fn_use_f(f: &Function) -> bool {
    f.params.iter().any(|p| matches!(&p.ty, DeclType::Plain(Type::Single)))
        || matches!(&f.ret, Some(DeclType::Plain(Type::Single)))
}

fn emit_device_fn(f: &Function) -> String {
    let use_f = fn_use_f(f);
    let mut body = f.body.clone();
    crate::transpiler::convert_returns(&mut body, &rust_name(&f.name));
    let ret = match &f.ret {
        Some(DeclType::Plain(t)) => cuda_c_type(*t),
        _ => "void",
    };
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let ty = match &p.ty {
                DeclType::Plain(t) => cuda_c_type(*t),
                _ => "long long",
            };
            format!("{ty} {}", rust_name(&p.name))
        })
        .collect();
    let mut src = format!(
        "__device__ {ret} {}({}) {{\n",
        cuda_fn_ident(&f.name),
        params.join(", ")
    );
    for s in &body {
        if let Some(text) = cuda_stmt(s, use_f, 1) {
            src.push_str(&text);
        }
    }
    src.push_str("}\n\n");
    src
}

/// CUDA C kernel for a 1-D device `Parallel For`. `bufs` is `(name, element)`.
pub fn kernel_c(var: &str, bufs: &[(String, Type)], body: &[Stmt]) -> String {
    let use_f = bufs.iter().any(|(_, t)| *t == Type::Single);
    let helpers = collect_helpers(body);
    let mut src = String::new();
    for f in &helpers {
        let ret = match &f.ret {
            Some(DeclType::Plain(t)) => cuda_c_type(*t),
            _ => "void",
        };
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| {
                let ty = match &p.ty {
                    DeclType::Plain(t) => cuda_c_type(*t),
                    _ => "long long",
                };
                format!("{ty} {}", rust_name(&p.name))
            })
            .collect();
        src.push_str(&format!(
            "__device__ {ret} {}({});\n",
            cuda_fn_ident(&f.name),
            params.join(", ")
        ));
    }
    if !helpers.is_empty() {
        src.push('\n');
    }
    for f in &helpers {
        src.push_str(&emit_device_fn(f));
    }
    let mut params: Vec<String> = bufs
        .iter()
        .map(|(n, t)| format!("{}* {}", cuda_c_type(*t), rust_name(n)))
        .collect();
    params.push("long long __from".into());
    params.push("long long __step".into());
    params.push("long long __n".into());
    src.push_str("extern \"C\" __global__ void k(");
    src.push_str(&params.join(", "));
    src.push_str(") {\n");
    src.push_str(
        "    long long __k = (long long)blockIdx.x * (long long)blockDim.x + (long long)threadIdx.x;\n",
    );
    src.push_str("    if (__k >= __n) return;\n");
    src.push_str(&format!(
        "    long long {} = __from + __k * __step;\n",
        rust_name(var)
    ));
    for s in body {
        if let Some(text) = cuda_stmt(s, use_f, 1) {
            src.push_str(&text);
        }
    }
    src.push_str("}\n");
    src
}

/// CUDA C kernel for nested `Parallel For y` / `x` — a 2-D grid.
pub fn kernel_c_2d(y: &str, x: &str, bufs: &[(String, Type)], body: &[Stmt]) -> String {
    let use_f = bufs.iter().any(|(_, t)| *t == Type::Single);
    TWO_D.with(|s| {
        *s.borrow_mut() = bufs.iter().map(|(n, _)| n.to_ascii_lowercase()).collect();
    });
    let helpers = collect_helpers(body);
    let mut src = String::new();
    for f in &helpers {
        let ret = match &f.ret {
            Some(DeclType::Plain(t)) => cuda_c_type(*t),
            _ => "void",
        };
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| {
                let ty = match &p.ty {
                    DeclType::Plain(t) => cuda_c_type(*t),
                    _ => "long long",
                };
                format!("{ty} {}", rust_name(&p.name))
            })
            .collect();
        src.push_str(&format!(
            "__device__ {ret} {}({});\n",
            cuda_fn_ident(&f.name),
            params.join(", ")
        ));
    }
    if !helpers.is_empty() {
        src.push('\n');
    }
    for f in &helpers {
        src.push_str(&emit_device_fn(f));
    }
    let mut params: Vec<String> = Vec::new();
    for (n, t) in bufs {
        let rn = rust_name(n);
        params.push(format!("{}* {rn}", cuda_c_type(*t)));
        params.push(format!("long long {rn}_cols"));
    }
    params.push("long long __yfrom".into());
    params.push("long long __ystep".into());
    params.push("long long __ny".into());
    params.push("long long __xfrom".into());
    params.push("long long __xstep".into());
    params.push("long long __nx".into());
    src.push_str("extern \"C\" __global__ void k(");
    src.push_str(&params.join(", "));
    src.push_str(") {\n");
    src.push_str(
        "    long long __ky = (long long)blockIdx.y * (long long)blockDim.y + (long long)threadIdx.y;\n",
    );
    src.push_str(
        "    long long __kx = (long long)blockIdx.x * (long long)blockDim.x + (long long)threadIdx.x;\n",
    );
    src.push_str("    if (__ky >= __ny || __kx >= __nx) return;\n");
    src.push_str(&format!(
        "    long long {} = __yfrom + __ky * __ystep;\n",
        rust_name(y)
    ));
    src.push_str(&format!(
        "    long long {} = __xfrom + __kx * __xstep;\n",
        rust_name(x)
    ));
    for s in body {
        if let Some(text) = cuda_stmt(s, use_f, 1) {
            src.push_str(&text);
        }
    }
    src.push_str("}\n");
    TWO_D.with(|s| s.borrow_mut().clear());
    src
}

fn is_two_d_buf(name: &str) -> bool {
    TWO_D.with(|s| s.borrow().contains(&name.to_ascii_lowercase()))
}

fn cuda_stmt(s: &Stmt, use_f: bool, indent: usize) -> Option<String> {
    let pad = "    ".repeat(indent);
    match s {
        Stmt::LineMark(_) | Stmt::Comment(_) => Some(String::new()),
        Stmt::Assign { target, value, op } => {
            let t = cuda_expr(target, use_f);
            let v = cuda_expr(value, use_f);
            let rhs = match op {
                None => v,
                Some(BinOp::Pow) if use_f => format!("powf((float)({t}), (float)({v}))"),
                Some(BinOp::Pow) => format!("pow((double)({t}), (double)({v}))"),
                Some(op) => format!("{t} {} {v}", cuda_bin(*op)),
            };
            Some(format!("{pad}{t} = {rhs};\n"))
        }
        Stmt::Dim {
            name,
            ty: DeclType::Plain(t),
            init: Some(e),
            ..
        } => Some(format!(
            "{pad}{} {} = {};\n",
            cuda_c_type(*t),
            rust_name(name),
            cuda_expr(e, use_f)
        )),
        Stmt::Dim {
            name,
            ty: DeclType::Plain(t),
            init: None,
            ..
        } => Some(format!("{pad}{} {};\n", cuda_c_type(*t), rust_name(name))),
        Stmt::Return(None) => Some(format!("{pad}return;\n")),
        Stmt::Return(Some(e)) => Some(format!("{pad}return {};\n", cuda_expr(e, use_f))),
        Stmt::If {
            branches,
            else_body,
        } => {
            let mut out = String::new();
            for (i, (cond, body)) in branches.iter().enumerate() {
                let head = if i == 0 { "if" } else { "} else if" };
                out.push_str(&format!(
                    "{pad}{head} ({}) {{\n",
                    cuda_expr(cond, use_f)
                ));
                for s in body {
                    if let Some(t) = cuda_stmt(s, use_f, indent + 1) {
                        out.push_str(&t);
                    }
                }
            }
            if let Some(body) = else_body {
                out.push_str(&format!("{pad}}} else {{\n"));
                for s in body {
                    if let Some(t) = cuda_stmt(s, use_f, indent + 1) {
                        out.push_str(&t);
                    }
                }
            }
            out.push_str(&format!("{pad}}}\n"));
            Some(out)
        }
        _ => None,
    }
}

fn cuda_expr(e: &Expr, use_f: bool) -> String {
    match &e.kind {
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float(n) => {
            let s = if n.fract() == 0.0 {
                format!("{:.1}", n)
            } else {
                n.to_string()
            };
            if use_f {
                format!("{s}f")
            } else {
                s
            }
        }
        ExprKind::Bool(true) => "1".into(),
        ExprKind::Bool(false) => "0".into(),
        ExprKind::Ident(n) => rust_name(n),
        ExprKind::Index(inner, idx) => {
            if let ExprKind::Index(base, row) = &inner.kind {
                if let ExprKind::Ident(n) = &base.kind {
                    if is_two_d_buf(n) {
                        let rn = rust_name(n);
                        return format!(
                            "{}[({}) * ({}_cols) + ({})]",
                            rn,
                            cuda_expr(row, use_f),
                            rn,
                            cuda_expr(idx, use_f)
                        );
                    }
                }
            }
            format!("{}[{}]", cuda_expr(inner, use_f), cuda_expr(idx, use_f))
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let l = cuda_expr(lhs, use_f);
            let r = cuda_expr(rhs, use_f);
            match op {
                BinOp::Pow if use_f => format!("powf((float)({l}), (float)({r}))"),
                BinOp::Pow => format!("pow((double)({l}), (double)({r}))"),
                _ => format!("({l} {} {r})", cuda_bin(*op)),
            }
        }
        ExprKind::Not(inner) => format!("(!({}))", cuda_expr(inner, use_f)),
        ExprKind::Cast(inner, t) => format!("(({})({}))", cuda_c_type(*t), cuda_expr(inner, use_f)),
        ExprKind::Try(inner) | ExprKind::Raw(inner) => cuda_expr(inner, use_f),
        ExprKind::Deref(inner) => cuda_expr(inner, use_f),
        ExprKind::Call { name, args } => cuda_call(name, args, use_f),
        _ => "0".into(),
    }
}

fn cuda_call(name: &str, args: &[Expr], use_f: bool) -> String {
    let a: Vec<String> = args.iter().map(|e| cuda_expr(e, use_f)).collect();
    let key = name.to_ascii_lowercase();
    if key == "iif" && a.len() == 3 {
        return format!("(({}) ? ({}) : ({}))", a[0], a[1], a[2]);
    }
    if is_device_math(name) {
        return cuda_math(&key, &a, use_f);
    }
    format!("{}({})", cuda_fn_ident(name), a.join(", "))
}

fn cuda_math(name: &str, a: &[String], use_f: bool) -> String {
    let x = a.first().map(|s| s.as_str()).unwrap_or("0");
    let f = |n: &str| {
        if use_f {
            format!("{n}f((float)({x}))")
        } else {
            format!("{n}((double)({x}))")
        }
    };
    match name {
        "sqr" => f("sqrt"),
        "abs" => {
            if use_f {
                format!("fabsf((float)({x}))")
            } else {
                format!("fabs((double)({x}))")
            }
        }
        "int" => f("floor"),
        "round" if a.len() >= 2 => {
            let p = &a[1];
            if use_f {
                format!(
                    "(roundf((float)({x}) * powf(10.0f, (float)({p}))) / powf(10.0f, (float)({p})))"
                )
            } else {
                format!(
                    "(round((double)({x}) * pow(10.0, (double)({p}))) / pow(10.0, (double)({p})))"
                )
            }
        }
        "round" => f("round"),
        "sin" => f("sin"),
        "cos" => f("cos"),
        "tan" => f("tan"),
        "atn" => f("atan"),
        "log" => f("log"),
        "exp" => f("exp"),
        _ => "0".into(),
    }
}

fn cuda_bin(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::Xor => "^",
        BinOp::Concat | BinOp::Pow => "+",
    }
}
