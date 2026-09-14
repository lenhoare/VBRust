//! `CudaBuffer<T>` and `CUDA.Upload` / `Alloc` / `Download`.
//!
//! CUDA names **where** work runs. `Parallel For` over those buffers is the GPU
//! claim; ordinary `Vec` Parallel For stays on CPU threads. There is no silent
//! host copy of a `Vec` onto the device.

use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::rust_name;

/// Why Python and C refuse CUDA rather than faking a host `Vec`.
pub const RUST_ONLY: &str = "`CudaBuffer` and `CUDA.Upload` are Rust-only. Python and C have no \
device backend, and we won't emit a host array that pretends otherwise. Run it with `vbr run`.";

/// Runtime for device buffers and kernel launch. Emitted only when the program
/// uses `CudaBuffer` / `CUDA.*`. Dynamically loads `libcuda` and `libnvrtc` —
/// no CUDA toolkit link, so CPU-only machines still `rustc` the program.
pub const CUDA_HELPER: &str = r#"
#[allow(dead_code, unused_mut, unused_variables, unused_assignments, unused_unsafe)]
struct __VbrCudaBuffer<T> {
    ptr: u64,
    len: usize,
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
}

impl<T> Drop for __VbrCudaBuffer<T> {
    fn drop(&mut self) {
        if self.ptr != 0 {
            let _ = __vbr_cuda_free(self.ptr);
            self.ptr = 0;
        }
    }
}

const __VBR_CUDA_NEEDED: &str = "CUDA needs an NVIDIA GPU and driver (libcuda). There is no silent \
CPU copy — ordinary Vec Parallel For stays on the CPU. Install a driver, or keep this work on a Vec.";

const __VBR_NVRTC_NEEDED: &str = "The GPU is there, but compiling a Parallel For kernel needs the \
CUDA toolkit (libnvrtc). Install the toolkit, or keep this loop on a Vec.";

fn __vbr_cuda_upload<T: Copy>(xs: &[T]) -> Result<__VbrCudaBuffer<T>, String> {
    let bytes = std::mem::size_of_val(xs);
    let ptr = __vbr_cuda_alloc_bytes(bytes)?;
    if bytes > 0 {
        __vbr_cuda_copy_hto_d(ptr, xs.as_ptr() as *const u8, bytes)?;
    }
    Ok(__VbrCudaBuffer {
        ptr,
        len: xs.len(),
        _t: std::marker::PhantomData,
    })
}

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
        _t: std::marker::PhantomData,
    })
}

fn __vbr_cuda_download<T: Copy + Default>(buf: &__VbrCudaBuffer<T>) -> Result<Vec<T>, String> {
    let mut out = vec![T::default(); buf.len];
    let bytes = std::mem::size_of_val(out.as_slice());
    if bytes > 0 {
        __vbr_cuda_copy_d_to_h(out.as_mut_ptr() as *mut u8, buf.ptr, bytes)?;
    }
    Ok(out)
}

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
    __vbr_cuda_launch(fun, grid, block, &mut args)
}

#[cfg(not(unix))]
fn __vbr_cuda_alloc_bytes(_: usize) -> Result<u64, String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(unix))]
fn __vbr_cuda_free(_: u64) -> Result<(), String> {
    Ok(())
}
#[cfg(not(unix))]
fn __vbr_cuda_copy_hto_d(_: u64, _: *const u8, _: usize) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(unix))]
fn __vbr_cuda_copy_d_to_h(_: *mut u8, _: u64, _: usize) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(unix))]
fn __vbr_cuda_memset(_: u64, _: usize) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(unix))]
fn __vbr_cuda_compile(_: &str) -> Result<u64, String> {
    Err(__VBR_CUDA_NEEDED.into())
}
#[cfg(not(unix))]
fn __vbr_cuda_launch(_: u64, _: u32, _: u32, _: &mut [*mut std::ffi::c_void]) -> Result<(), String> {
    Err(__VBR_CUDA_NEEDED.into())
}

#[cfg(unix)]
mod __vbr_cuda_drv {
    #![allow(dead_code, unused_unsafe)]
    use std::collections::HashMap;
    use std::ffi::{CString, c_char, c_int, c_uint, c_void};
    use std::sync::{Mutex, OnceLock};

    const RTLD_NOW: c_int = 2;

    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    struct Api {
        cu_init: unsafe extern "C" fn(c_uint) -> c_int,
        cu_device_get: unsafe extern "C" fn(*mut c_int, c_int) -> c_int,
        cu_ctx_create: unsafe extern "C" fn(*mut usize, c_uint, c_int) -> c_int,
        cu_mem_alloc: unsafe extern "C" fn(*mut u64, usize) -> c_int,
        cu_mem_free: unsafe extern "C" fn(u64) -> c_int,
        cu_memcpy_htod: unsafe extern "C" fn(u64, *const c_void, usize) -> c_int,
        cu_memcpy_dtoh: unsafe extern "C" fn(*mut c_void, u64, usize) -> c_int,
        cu_memset: unsafe extern "C" fn(u64, c_uint, usize) -> c_int,
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
    }

    unsafe impl Send for Api {}
    unsafe impl Sync for Api {}

    fn load_lib(names: &[&str]) -> Result<*mut c_void, ()> {
        for n in names {
            let c = CString::new(*n).map_err(|_| ())?;
            let h = unsafe { dlopen(c.as_ptr(), RTLD_NOW) };
            if !h.is_null() {
                return Ok(h);
            }
        }
        Err(())
    }

    unsafe fn sym(h: *mut c_void, names: &[&str]) -> Result<*mut c_void, String> {
        for n in names {
            let c = CString::new(*n).unwrap();
            let p = dlsym(h, c.as_ptr());
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
        let cuda = load_lib(&["libcuda.so.1", "libcuda.so"])
            .map_err(|_| super::__VBR_CUDA_NEEDED.to_string())?;
        let nvrtc = load_lib(&[
            "libnvrtc.so.12",
            "libnvrtc.so.13",
            "libnvrtc.so.11",
            "libnvrtc.so",
        ])
        .map_err(|_| super::__VBR_NVRTC_NEEDED.to_string())?;
        let cu_init = std::mem::transmute(sym(cuda, &["cuInit"])?);
        let cu_device_get = std::mem::transmute(sym(cuda, &["cuDeviceGet"])?);
        let cu_ctx_create =
            std::mem::transmute(sym(cuda, &["cuCtxCreate_v2", "cuCtxCreate"])?);
        let cu_mem_alloc = std::mem::transmute(sym(cuda, &["cuMemAlloc_v2", "cuMemAlloc"])?);
        let cu_mem_free = std::mem::transmute(sym(cuda, &["cuMemFree_v2", "cuMemFree"])?);
        let cu_memcpy_htod =
            std::mem::transmute(sym(cuda, &["cuMemcpyHtoD_v2", "cuMemcpyHtoD"])?);
        let cu_memcpy_dtoh =
            std::mem::transmute(sym(cuda, &["cuMemcpyDtoH_v2", "cuMemcpyDtoH"])?);
        let cu_memset = std::mem::transmute(sym(cuda, &["cuMemsetD8_v2", "cuMemsetD8"])?);
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
        let api = Api {
            cu_init,
            cu_device_get,
            cu_ctx_create,
            cu_mem_alloc,
            cu_mem_free,
            cu_memcpy_htod,
            cu_memcpy_dtoh,
            cu_memset,
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
        };
        check((api.cu_init)(0), "cuInit")?;
        let mut dev: c_int = 0;
        check((api.cu_device_get)(&mut dev, 0), "cuDeviceGet")?;
        let mut ctx: usize = 0;
        check((api.cu_ctx_create)(&mut ctx, 0, dev), "cuCtxCreate")?;
        let _ = ctx;
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
        grid: u32,
        block: u32,
        args: &mut [*mut c_void],
    ) -> Result<(), String> {
        let api = api()?;
        check(
            unsafe {
                (api.cu_launch)(
                    fun,
                    grid,
                    1,
                    1,
                    block,
                    1,
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

#[cfg(unix)]
fn __vbr_cuda_alloc_bytes(n: usize) -> Result<u64, String> {
    __vbr_cuda_drv::alloc_bytes(n)
}
#[cfg(unix)]
fn __vbr_cuda_free(ptr: u64) -> Result<(), String> {
    __vbr_cuda_drv::free(ptr)
}
#[cfg(unix)]
fn __vbr_cuda_copy_hto_d(dst: u64, src: *const u8, n: usize) -> Result<(), String> {
    __vbr_cuda_drv::copy_hto_d(dst, src, n)
}
#[cfg(unix)]
fn __vbr_cuda_copy_d_to_h(dst: *mut u8, src: u64, n: usize) -> Result<(), String> {
    __vbr_cuda_drv::copy_d_to_h(dst, src, n)
}
#[cfg(unix)]
fn __vbr_cuda_memset(ptr: u64, n: usize) -> Result<(), String> {
    __vbr_cuda_drv::memset(ptr, n)
}
#[cfg(unix)]
fn __vbr_cuda_compile(src: &str) -> Result<u64, String> {
    __vbr_cuda_drv::compile(src).map(|p| p as u64)
}
#[cfg(unix)]
fn __vbr_cuda_launch(
    fun: u64,
    grid: u32,
    block: u32,
    args: &mut [*mut std::ffi::c_void],
) -> Result<(), String> {
    __vbr_cuda_drv::launch(fun as usize, grid, block, args)
}
"#;

pub fn is_cuda_ns(name: &str) -> bool {
    name.eq_ignore_ascii_case("cuda")
}

pub fn is_cuda_method(method: &str) -> bool {
    matches!(
        method.to_ascii_lowercase().replace('_', "").as_str(),
        "upload" | "alloc" | "download"
    )
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
        | ExprKind::ParallelSum(inner)
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
        | ExprKind::ParallelSum(inner)
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
        | ExprKind::ParallelSum(inner)
        | ExprKind::TupleIndex(inner, _)
        | ExprKind::Closure { body: inner, .. } => walk_expr_idents(inner, allow, out),
        _ => {}
    }
}

/// CUDA Parallel For body this slice: assignments and `Dim` locals only.
pub fn check_device_body(stmts: &[Stmt], line: usize, diags: &mut Diagnostics) -> bool {
    for s in stmts {
        match s {
            Stmt::LineMark(_) | Stmt::Comment(_) => {}
            Stmt::Assign { .. } => {}
            Stmt::Dim { .. } => {}
            Stmt::For { parallel: true, .. } => {
                diags.error(
                    line,
                    "CUDA buffers are a 1-D index space (`buf[i]`). Nested \
                     `Parallel For y` / `Parallel For x` stays on CPU `Vec`s — \
                     a 2-D CUDA grid is later.",
                );
                return false;
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
                        "CUDA `Parallel For` body is assignments and `Dim` locals \
                         this slice (`{}` isn't).",
                        stmt_kind(other)
                    ),
                );
                return false;
            }
        }
        if stmt_has_host_expr(s) {
            diags.error(
                line,
                "CUDA `Parallel For` body is arithmetic on `buf[i]` — no method \
                 calls, no host functions. Compute on the device with `a[i] * 2.0`.",
            );
            return false;
        }
    }
    true
}

fn stmt_has_host_expr(s: &Stmt) -> bool {
    match s {
        Stmt::Assign { target, value, .. } => expr_has_host(target) || expr_has_host(value),
        Stmt::Dim { init: Some(e), .. } => expr_has_host(e),
        _ => false,
    }
}

fn expr_has_host(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::MethodCall { .. } | ExprKind::Call { .. } | ExprKind::ParallelSum(_) | ExprKind::Str(_) => {
            true
        }
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

/// CUDA C kernel for a 1-D device `Parallel For`. `bufs` is `(name, element)`.
pub fn kernel_c(var: &str, bufs: &[(String, Type)], body: &[Stmt]) -> String {
    let use_f = bufs.iter().any(|(_, t)| *t == Type::Single);
    let mut params: Vec<String> = bufs
        .iter()
        .map(|(n, t)| format!("{}* {}", cuda_c_type(*t), rust_name(n)))
        .collect();
    params.push("long long __from".into());
    params.push("long long __step".into());
    params.push("long long __n".into());
    let mut src = String::new();
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
        if let Some(line) = cuda_stmt(s, use_f) {
            src.push_str(&format!("    {line}\n"));
        }
    }
    src.push_str("}\n");
    src
}

fn cuda_stmt(s: &Stmt, use_f: bool) -> Option<String> {
    match s {
        Stmt::Assign { target, value, .. } => Some(format!(
            "{} = {};",
            cuda_expr(target, use_f),
            cuda_expr(value, use_f)
        )),
        Stmt::Dim {
            name,
            ty: DeclType::Plain(t),
            init: Some(e),
            ..
        } => Some(format!(
            "{} {} = {};",
            cuda_c_type(*t),
            rust_name(name),
            cuda_expr(e, use_f)
        )),
        Stmt::Dim {
            name,
            ty: DeclType::Plain(t),
            init: None,
            ..
        } => Some(format!("{} {};", cuda_c_type(*t), rust_name(name))),
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
