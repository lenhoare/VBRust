# qwenprojects — VBR language-testing projects

Built by an assistant to shake out the VBR (Bust) language by writing real
programs in it. No Rust/transpiler source was changed; every problem met was
either worked around in-language or skipped, and logged in **FINDINGS.md**
(F-001 … F-032, dated, kinded bug/quirk/ergonomic/positive, with repros).

## How to run

The binary used: `/home/len/dev/VBRust/target/debug/vbr`.

```
vbr run <file.vbr>              # single-file, core language only
vbr runproject <folder>         # project build + run (stdlib / crates / GUI / GPU)
vbr test <folder>               # run the project's Test blocks
```

Terminal animations (lissajous, cellab, boids) redraw with ANSI escapes —
run them in a real terminal. GUI/GPU projects open a window (WSLg).

## Warmups (core language)

| Project | What it exercises |
|---|---|
| `bottles.vbr` | `For..Step -1`, string building, `&` |
| `guess/` | `Rnd()` absence → `Use rand` + inline Rust; `InputBox`; `Handle`+`Continue` |
| `caesar.vbr` | `Asc`/`Chr`/`Mid`/`Len`/`Mod`; negative shift |
| `calendar.vbr` | `Match`, `Int()` truncation, Vec literals, missing `Space()` |
| `textstat/` | `FileSystem`, `HashMap`, Type, Vec index assign, selection sort |
| `calc/` | recursive-descent parser, `Option`, methods, `.test.vbr` + `vbr test`, `vbr py`/`vbr c` cross-target |

## The ten larger projects

| Project | Surface | What it is / exercises |
|---|---|---|
| `mandel.vbr` | terminal | Mandelbrot + Julia escape-time fractals |
| `chaos.vbr` | terminal | logistic bifurcation + Hénon attractor, density grids |
| `pascal.vbr` | terminal | Pascal's triangle + mod-2 Sierpiński |
| `primes/` | terminal + tests | sieve, Goldbach (ByRef out-params), Collatz |
| `lissajous.vbr` | terminal | animated Lissajous curves, `Sleep`, ANSI |
| `cellab/` | terminal | cellular automata from B/S rule strings (Life/Day&Night/Maze) |
| `boids.vbr` | terminal | flocking, O(n²), Vec<Type> |
| `amortize/` | **GUI form** | loan calculator Window; logic split into tested `loan.vbr` |
| `juliagpu/` | **GPU Sketch** | animated Julia set kernel, cosine palette |
| `greyscott/` | **GPU Sketch** | Gray–Scott reaction–diffusion via `Sample(frame,…)` feedback |

## Round 2 — larger projects (after fixes landed)

Regression sweep first: unary-minus-on-Double fixed (F-013), `Str()` added;
Len/Mid unit mismatch, InStr zero-base, ByRef-init, variable Step, f64 For
bounds, `Double ^ Long`, and the py/C string builtins were still open at sweep
time (see FINDINGS.md, "Round 2" section).

| Project | Surface | What it is |
|---|---|---|
| `controlroom/` | **GUI** | mission-control dashboard: Tabs, Frames, ProgressBars, Slider, Radio, Toggler, Table, Tooltip, Markdown — Dracula |
| `recipes/` | **GUI** | recipe box: List, TextInput, Chooser, Checkbox, Slider, TextArea, Tooltip + JSON persistence (dynamic Json API) — CatppuccinMocha |
| `wavebench/` | **GUI** | additive-synthesis lab: Canvas plots the sum of 3 harmonics live from sliders, presets for square/saw — TokyoNightStorm |
| `fourier.vbr` | terminal | animated epicycles drawing a square wave |
| `newton.vbr` | terminal | Newton fractal for z³−1, basin-shaded |
| `wireworld.vbr` | terminal | 4-state circuit CA: two clock loops into an OR junction |
| `ecology.vbr` | terminal | fox/rabbit/grass predator-prey with live sparklines |
| `tunnelgpu/` | **GPU Sketch** | demoscene tunnel (Atan2 + Noise) |
| `raymarch/` | **GPU Sketch** | raymarched sphere on a checker floor, lit |
| `orbitsgpu/` | **GPU Sketch** | three Lissajous particles with decaying trails |

Run the GPU sketches **one at a time** — two concurrent wgpu windows kill each
other on this WSLg box (F-041).

## Headline results

- **A lot worked first try:** terminal maths/fractals, the GUI form, and both GPU
  sketches all ran on the first (or second) compile. Error messages and teaching
  lines are generally excellent — the `Rnd()` refusal even hands you the fix.
- **Recurring themes in FINDINGS.md:**
  - String builtins disagree about units (`Len` counts bytes, `Mid` counts chars —
    F-006) and are missing from the py/C targets (F-015).
  - VB-familiar gaps: `Split`, `Join`, `Space`, `Atn`, `Format` (F-008/F-011/F-022).
  - Numeric seams: `Double ^ Long` (F-027), f64 `For` bounds (F-020), variable
    `Step` (F-019), unary minus on Double (F-013), `Dim x As Long` not initialised
    for a ByRef first use (F-018).
  - 1-based muscle memory: `InStr` returns an Option AND a zero-based position (F-025).
  - GPU kernels differ invisibly from normal code: reserved names (F-030) and a
    `Dim`+`For` redefinition (F-031).

See FINDINGS.md for the full, dated record.
