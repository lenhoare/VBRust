# Vinyl Parallel Specification

Independent loops and NVIDIA GPU buffers. **Niche, Rust-only.** Ordinary
`For` / `Vec` programs do not need this. Python, C, and the Android interpreter
refuse it rather than looping sequentially or faking a host array.

> **Parallel names independence. CUDA names where the work runs.** A
> `Parallel For` over ordinary `Vec`s is CPU threads. The same loop over
> `CudaBuffer`s is the GPU. There is no silent copy of a `Vec` onto the device.

The compiler help category **Parallel** (`vbr help`) is the same surface as this
file. Design notes that led here live in `old/parallel.md`.

---

## 1. `Parallel For`

A counting loop whose iterations are independent. It is a different claim from
ordinary `For` (which may accumulate: `total = total + i`).

```vb
Parallel For i = 0 To xs.Len() - 1
    out[i] = xs[i] * xs[i]
Next
```

Each iteration may **read** anything. Two iterations may not **write** the same
slot. Writes are `arr[i]` for the loop variable. A shared scalar
(`total = total + xs[i]`) is a compile error — that is `Parallel Sum xs`.
Reading a *different* array at a neighbour (`xs[i + 1]` while writing `out[i]`)
is fine.

Nested `Parallel For y` wrapping `Parallel For x` is a 2-D index space
(`arr[y][x]`). On `Vec`s that is one CPU launch over `ny * nx`, not a thread
pool per row. A sequential `For x` inside `Parallel For y` can write
`arr[y][x]` too (the inner loop stays sequential).

Rejected in the body: `Exit For`, `Continue`, `Return`, a third nested
`Parallel For`, mutating methods (`Push` / `Pop`), a variable or floating
`Step`. `Dim` inside the body is a per-iteration local.

Slots must exist before you write `dest[i]`. There is no `ReDim` / `Resize`:
start dest with a fill — `Dim dest As Vec<Long> = [0; n]` — then write the
slots. A log-depth sum is several of those rounds (`examples/parallel_sum.vbr`).

`Parallel` is a reserved word. `Sum` is not — `.Sum()` and `Dim Sum` still work.

`examples/parallel_for_2d.vbr` is the CPU nest.

---

## 2. `Parallel Sum`

The first-class reduction that `total = total + xs[i]` is not.

```vb
Dim total As Long = Parallel Sum xs
```

On a numeric `Vec` or array, CPU threads add chunks, then the partials combine
in order. Empty input is `0`. On a 1-D `CudaBuffer` the same spelling reduces
on the GPU — there is no silent download. A 2-D `CudaBuffer<CudaBuffer<T>>` is
an error (that is nested `Parallel For y` / `x`).

It is an expression, not a statement. Inside `Parallel For` it is an error.
A device sum can fail (no GPU) and uses implicit `?` like `CUDA.Download`.

`examples/parallel_sum_expr.vbr` prints `36` for `[1..8]`.
`examples/cuda_sum.vbr` is the device form.

---

## 3. CUDA — where, not a second language

`CUDA.Upload` / `Alloc` / `Managed` / `Download` copy numeric lists onto an
NVIDIA GPU and back. A `Parallel For` that indexes those buffers is the GPU
claim. Mixing a host `Vec` with a `CudaBuffer` in one loop is an error, not a
silent upload. Mixing 1-D and 2-D buffers in one loop is an error too.

`CUDA` is not a `vbr_stdlib` crate. The compiler emits a dynamic loader
(`libcuda` / `libnvrtc` on Linux, `nvcuda.dll` / `nvrtc64_*.dll` on Windows,
plus `%CUDA_PATH%\bin` for the compiler) only when the program uses it. CPU-only
machines still `rustc` the program; a missing driver fails at run time with a
teaching error.

Element type comes from `Dim … As CudaBuffer<T>` — not `Alloc<Single>` /
`Managed<Single>`, which the parser would read as a comparison.

Calls that can fail (no GPU, out of memory) use implicit `?` like
`FileSystem.Read`.

### `CudaBuffer<T>`

A 1-D device array of numbers (`Integer` / `Long` / `Single` / `Double` /
`Byte`). `CudaBuffer<CudaBuffer<T>>` is a rectangular 2-D grid — one flat
allocation, not an array of device pointers. There is no empty `CudaBuffer` and
no `.Push`.

On the host: `.Len()` / `.Count()` (length, or rows) and `.Cols()` (columns; 0
if 1-D). Indexing `buf[i]` or `buf[y][x]` is a kernel read or write inside
`Parallel For`. On the host, `CUDA.Download` first — unless the buffer came
from `CUDA.Managed`.

A `CudaBuffer` **parameter** is never treated as managed (the callee does not
know how it was created).

### Upload / Alloc / Download

```vb
Dim a As CudaBuffer<Single> = CUDA.Upload(input)
Dim b As CudaBuffer<Single> = CUDA.Alloc(n)
Parallel For i = 0 To n - 1
    b[i] = a[i] * a[i] + 1.0
Next
Dim result As Vec<Single> = CUDA.Download(b)
```

`CUDA.Upload` of a `Vec<Vec<T>>` is 2-D. `CUDA.Alloc(rows, cols)` needs
`Dim … As CudaBuffer<CudaBuffer<T>>`. Nested `Parallel For y` / `x` over those
buffers is **one** 2-D CUDA launch (`grid.x` × `grid.y`) — the inner loop is
not a second kernel.

`examples/cuda_upload.vbr` is the 1-D path. `examples/cuda_grid.vbr` is the 2-D
nest.

### Managed memory

`CUDA.Managed` returns the **same** `CudaBuffer` type, not a `Vec` that magically
lives on the GPU. Host index is allowed because the pages can sit on both sides.
Movement is explicit:

```vb
Dim a As CudaBuffer<Long> = CUDA.Managed(n)
a[0] = 1
CUDA.Prefetch(a)          ' pages toward the GPU (optional)
Parallel For i = 0 To n - 1
    a[i] = a[i] * 2
Next
CUDA.PrefetchHost(a)      ' pages toward the CPU (optional)
CUDA.Sync()               ' wait for the GPU and outstanding prefetch
Debug.Print a[0]
```

Prefetch is **not required for correctness** (unified memory can page-fault).
It is the honest movement. `CUDA.Prefetch` / `PrefetchHost` on an Upload/Alloc
buffer is a compile error. `CUDA.Sync()` takes no arguments.

2-D: `CUDA.Managed(rows, cols)` with `Dim … As CudaBuffer<CudaBuffer<T>>`.
Host index is `buf[y][x]` (not a row `buf[y]`).

`examples/cuda_managed.vbr` is the 1-D path.

### Device helpers

A numeric `Function` (ByVal numbers in, a number out — `If` / `Return` /
arithmetic) can be called from a CUDA `Parallel For`; it still exists as Rust
on the host. `Sin` / `Sqr` / `Abs` / `IIf` work in the loop too.
`examples/cuda_call.vbr` is `Clamp(Sq(a[i]), …)`.

`examples/cuda_dot.vbr` is a GPU multiply then `Parallel Sum` of that buffer
(no Download). `examples/cuda_cross.vbr` is a 3-vector cross product — reads of
a *different* buffer may use any index.

This is not Gpu Draw (WGSL / `Gpu Function`). CUDA is the NVIDIA compute path.

---

## 4. Examples

| File | What it is |
|------|------------|
| `examples/parallel_for_2d.vbr` | CPU nested `Parallel For` |
| `examples/parallel_sum.vbr` | Log-depth sum as several `Parallel For` rounds |
| `examples/parallel_sum_expr.vbr` | `Parallel Sum xs` on a `Vec` |
| `examples/cuda_upload.vbr` | Upload / Alloc / kernel / Download |
| `examples/cuda_grid.vbr` | 2-D CUDA nest |
| `examples/cuda_call.vbr` | Numeric `Function` from the kernel |
| `examples/cuda_sum.vbr` | `Parallel Sum` on a `CudaBuffer` |
| `examples/cuda_dot.vbr` | GPU multiply + device sum |
| `examples/cuda_cross.vbr` | 3-vector cross product |
| `examples/cuda_managed.vbr` | Host index + Prefetch / Sync |
