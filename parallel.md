Worked for 26s

Yes. And I think there is a very natural VBR version of this.

The key design choice I’d make is:

> Parallel VBR describes the computation; CUDA describes where it runs.



So I would resist exposing blocks, grids, threadIdx.x, shared-memory barriers etc. in ordinary VBR. Inline CUDA/Rust can be the escape hatch later.

The DataFrame work actually gives us a precedent: inside a column formula, ordinary VBR expressions acquire a parallel, elementwise meaning. For general GPU computation I think we want the equivalent concept, but explicit enough that dependencies are obvious.

My starting point would be something like:

Dim a = CUDA.Upload(input)
Dim b = CUDA.Alloc<Single>(a.Count)

Parallel For i = 0 To a.Count - 1
    b(i) = a(i) * a(i) + 1.0
Next

Dim result = CUDA.Download(b)

That is almost boring — which I think is excellent.

The compiler knows that the Parallel For iterations must be independent. It can generate essentially:

i = CUDA global thread index
if i < n:
    b[i] = a[i] * a[i] + 1

There are then several very interesting things we can build on top of that without turning VBR into CUDA C.

1. Make memory movement visible

I wouldn't silently copy ordinary Vecs to the GPU when entering Parallel For. That's convenient initially but teaches a dreadful performance model.

Something conceptually like:

Dim cpuData As Vec<Single> = ...
Dim gpuData = CUDA.Upload(cpuData)

...

cpuData = CUDA.Download(gpuData)

fits VBR's existing philosophy extremely well: convenient, but honest about what's underneath.

Perhaps eventually unified memory could exist:

Dim a = CUDA.Managed<Single>(1000000)

but it shouldn't be the conceptual foundation.

2. Parallel For should have strong semantics

The beautiful rule could simply be:

> Each iteration may read anything available to it, but two iterations may not write the same location unless the operation is explicitly parallel-safe.



Thus:

Parallel For i = 0 To n - 1
    output(i) = input(i) * 2
Next

obviously valid.

This:

Parallel For i = 0 To n - 1
    total = total + input(i)
Next

should be a compile error.

And instead VBR gives you:

Dim total = Parallel Sum input

or perhaps:

Dim total = input.ParallelSum()

I prefer having reductions as first-class operations rather than making the programmer understand atomics just to add numbers.

3. Then arrays become much more interesting than individual threads

You could get to:

Parallel For i = 1 To n - 2
    output(i) =
        input(i - 1) +
        input(i) +
        input(i + 1)
Next

That's a perfectly intelligible stencil computation. The compiler worries about launch dimensions.

Likewise:

Parallel For y = 0 To height - 1
    For x = 0 To width - 1
        image(x, y) = Mandelbrot(x, y)
    Next
Next

We might even permit:

Parallel For y = ...
    Parallel For x = ...

and let dimensionality naturally become CUDA's 2D grid.

4. Functions called from parallel code are the really nice part

I'd want this:

Function Mandelbrot(x As Long, y As Long) As Long
    ...
End Function

Parallel For y = 0 To height - 1
    Parallel For x = 0 To width - 1
        image(x, y) = Mandelbrot(x, y)
    Next
Next

The compiler sees that Mandelbrot is reachable from GPU code and compiles a device version.

That feels far more like a language with a CUDA backend than a CUDA wrapper.

And there's an interesting consequence: VBR could diagnose things that CUDA C++ leaves comparatively obscure:

Mandelbrot cannot run on CUDA:
  → calls FileSystem.ReadText()
  → FileSystem is host-only

That would be excellent.

5. I would deliberately postpone warps/shared memory

Unlike WarpC, where the warp is the programming model, I don't think ordinary Parallel VBR should initially know that warps exist.

Start with:

GPU arrays

upload/download

Parallel For

pure/device-compatible functions

multidimensional iteration

reductions

perhaps atomics


That alone gives you an enormous language: image processing, Monte Carlo, cellular automata, particle simulations, numerical algorithms, genetic populations, neural-net primitives, etc.

Then we could introduce an advanced CUDA layer later:

CUDA Kernel ...

with shared memory, synchronization, warp operations and so forth.

The separation would be very clean:

VBR → ordinary sequential programming

Parallel VBR → machine-independent data parallelism

CUDA → one implementation target for Parallel VBR

CUDA-specific VBR / inline CUDA → hardware-aware optimisation

The part I particularly like is that Parallel For isn't really a CUDA feature. We could eventually target CPU threads, SIMD, WebGPU, Metal, Vulkan compute, etc. The program says these iterations are independent. CUDA is merely very good at exploiting that statement.

I think that's the conceptual foundation I'd build from. And there are some genuinely interesting language-design questions immediately after this — particularly what variables are allowed to cross into a Parallel For, and how VBR's Rust ownership semantics can actually help us prove race freedom at compile time.
