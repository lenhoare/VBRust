//! `Gpu Draw` / `Gpu Function` → WGSL fragment shader + an iced `Shader` widget.
//!
//! A pixel kernel (`For y` / `For x` / `Set Pixel`) becomes one fragment.
//! `Copy` / `Clear` are extra passes onto an offscreen paper; `frame` is last
//! paper. `Into spr` writes a named `Pixels` instead. `Using mask` samples a
//! second texture. Helpers marked `Gpu Function` are emitted as WGSL
//! functions. CPU `Draw` (Text / Fill / Stroke) stays on a canvas overlay.

use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::rust_name;

/// Compile a `Gpu Draw` body plus same-file `Gpu Function`s to WGSL, then wrap
/// it in the iced shader runtime. `state` fields become uniform floats.
pub fn emit_shader_program(
    sketch_name: &str,
    gpu_draw: &[Stmt],
    gpu_fns: &[&Function],
    state: &[StateField],
    constants: &[ConstDef],
    diags: &mut Diagnostics,
) -> Option<String> {
    let passes = split_gpu_cmds(gpu_draw, state, diags)?;
    if passes.is_empty() {
        diags.error_once(
            "gpu-draw-empty",
            "`Gpu Draw` needs a pixel kernel (`For y` / `For x` / `Set Pixel`) or `Copy` / `Clear`.",
        );
        return None;
    }
    for f in state {
        let r = rust_name(&f.name);
        if (is_gpu_uniform(&f.ty) || is_pixels(&f.ty)) && RESERVED_UNIFORM.contains(&r.as_str()) {
            diags.error_once(
                "gpu-uniform-name",
                format!(
                    "State field `{r}` collides with a Gpu Draw uniform name. Pick another name."
                ),
            );
            return None;
        }
        if r == "frame" {
            diags.error_once(
                "gpu-frame-name",
                "`frame` is the last GPU paper — pick another State name.",
            );
            return None;
        }
    }

    let uniforms: HashSet<String> = state
        .iter()
        .filter(|f| is_gpu_uniform(&f.ty))
        .map(|f| rust_name(&f.name))
        .collect();

    let mut wgsl = String::new();
    wgsl.push_str("struct Uniforms {\n    origin: vec2<f32>,\n    scale: f32,\n    _pad: f32,\n    size: vec2<f32>,\n");
    for f in state {
        if is_gpu_uniform(&f.ty) {
            wgsl.push_str(&format!("    {}: f32,\n", rust_name(&f.name)));
        }
        if is_pixels(&f.ty) {
            let r = rust_name(&f.name);
            wgsl.push_str(&format!("    {r}_w: f32,\n    {r}_h: f32,\n"));
        }
    }
    wgsl.push_str("}\n@group(0) @binding(0) var<uniform> u: Uniforms;\n");
    wgsl.push_str("@group(1) @binding(0) var src_tex: texture_2d<f32>;\n");
    wgsl.push_str("@group(1) @binding(1) var src_samp: sampler;\n");
    wgsl.push_str("@group(1) @binding(2) var mask_tex: texture_2d<f32>;\n\n");

    for c in constants {
        if is_gpu_numeric_type(&c.ty) {
            let n = rust_name(&c.name);
            let v = wgsl_expr(&c.value, &HashSet::new(), diags)?;
            wgsl.push_str(&format!("const {n}: f32 = {v};\n"));
        }
    }
    if constants.iter().any(|c| is_gpu_numeric_type(&c.ty)) {
        wgsl.push('\n');
    }

    for f in gpu_fns {
        match wgsl_gpu_fn(f, &uniforms, diags) {
            Some(s) => {
                wgsl.push_str(&s);
                wgsl.push('\n');
            }
            None => return None,
        }
    }

    wgsl.push_str(
        "@vertex\nfn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {\n    \
         var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));\n    \
         return vec4(p[i], 0.0, 1.0);\n}\n\n",
    );

    let paper_has_copy = passes.iter().any(|p| match p {
        GpuPass::Copy(c) => c.target.is_none(),
        _ => false,
    });
    let mut copy_idx = 0usize;
    let mut kernel_idx = 0usize;
    let mut runtime_passes: Vec<RuntimePass> = Vec::new();
    for pass in &passes {
        match pass {
            GpuPass::Clear { color, target } => {
                let col = wgsl_color(color, &uniforms, diags)?;
                let name = format!("fs_clear_{}", runtime_passes.len());
                wgsl.push_str(&format!(
                    "@fragment\nfn {name}(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{\n    \
                     _ = pos;\n    return {col};\n}}\n\n"
                ));
                runtime_passes.push(RuntimePass::Clear {
                    fs: name,
                    target: target.clone(),
                });
            }
            GpuPass::Kernel { x, y, body, target } => {
                let xn = rust_name(x);
                let yn = rust_name(y);
                let composite = match target {
                    None => paper_has_copy,
                    Some(t) => passes.iter().any(|p| match p {
                        GpuPass::Copy(c) => c.target.as_deref() == Some(t.as_str()),
                        _ => false,
                    }),
                };
                let init_a = if composite { "0.0" } else { "1.0" };
                let fs = format!("fs_kernel_{kernel_idx}");
                kernel_idx += 1;
                wgsl.push_str(&format!(
                    "@fragment\nfn {fs}(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{\n    \
                     let logical = pos.xy / u.scale;\n    \
                     let {xn} = logical.x;\n    \
                     let {yn} = logical.y;\n    \
                     if {xn} < 0.0 || {yn} < 0.0 || {xn} >= u.size.x || {yn} >= u.size.y {{ discard; }}\n    \
                     var col = vec4<f32>(0.0, 0.0, 0.0, {init_a});\n"
                ));
                for s in body {
                    wgsl.push_str(&wgsl_stmt(s, 1, &uniforms, diags)?);
                }
                if composite {
                    wgsl.push_str("    if col.a < 0.001 { discard; }\n");
                }
                wgsl.push_str("    return col;\n}\n\n");
                runtime_passes.push(RuntimePass::Kernel {
                    fs,
                    target: target.clone(),
                });
            }
            GpuPass::Copy(copy) => {
                let fs = format!("fs_copy_{copy_idx}");
                let body = wgsl_copy_fs(copy, state, &uniforms, diags)?;
                wgsl.push_str(&format!(
                    "@fragment\nfn {fs}(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{\n{body}}}\n\n"
                ));
                runtime_passes.push(RuntimePass::Copy {
                    fs: fs.clone(),
                    src: copy.src.clone(),
                    blend: copy.blend,
                    mask: copy.mask.clone(),
                    target: copy.target.clone(),
                });
                copy_idx += 1;
            }
        }
    }
    wgsl.push_str(
        "@fragment\nfn fs_blit(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {\n    \
         let logical = (pos.xy - u.origin) / u.scale;\n    \
         if logical.x < 0.0 || logical.y < 0.0 || logical.x >= u.size.x || logical.y >= u.size.y { discard; }\n    \
         let uv = logical / u.size;\n    \
         return textureSample(src_tex, src_samp, uv);\n}\n",
    );

    Some(rust_runtime(sketch_name, &wgsl, state, &runtime_passes))
}

pub(crate) fn is_gpu_uniform(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(t) if is_gpu_numeric_type(t))
}

pub(crate) fn is_pixels(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Named(n) if n.eq_ignore_ascii_case("pixels"))
}

fn is_gpu_numeric_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Integer | Type::Long | Type::LongLong | Type::Single | Type::Double | Type::Byte
    )
}

const RESERVED_UNIFORM: &[&str] = &[
    "origin", "scale", "size", "_pad", "u", "col", "pos", "logical", "frame",
];

enum GpuPass {
    Clear { color: Expr, target: Option<String> },
    Kernel { x: String, y: String, body: Vec<Stmt>, target: Option<String> },
    Copy(CopyCmd),
}

struct CopyCmd {
    src: String,
    args: Vec<Expr>,
    mask: Option<String>,
    color_key: Option<Expr>,
    blend: GpuBlend,
    target: Option<String>,
}

enum RuntimePass {
    Clear { fs: String, target: Option<String> },
    Kernel { fs: String, target: Option<String> },
    Copy {
        fs: String,
        src: String,
        blend: GpuBlend,
        mask: Option<String>,
        target: Option<String>,
    },
}

fn split_gpu_cmds(stmts: &[Stmt], state: &[StateField], diags: &mut Diagnostics) -> Option<Vec<GpuPass>> {
    flatten_gpu_cmds(stmts, None, state, diags)
}

fn flatten_gpu_cmds(
    stmts: &[Stmt],
    target: Option<String>,
    state: &[StateField],
    diags: &mut Diagnostics,
) -> Option<Vec<GpuPass>> {
    let stmts = skip_noise(stmts);
    if stmts.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for s in &stmts {
        match s {
            Stmt::GpuInto { name, body } => {
                if target.is_some() {
                    diags.error_once(
                        "gpu-into-nested",
                        "`Into` doesn't nest — finish one `Pixels` before opening another.",
                    );
                    return None;
                }
                let n = rust_name(name);
                if n == "frame" {
                    diags.error_once(
                        "gpu-into-frame",
                        "`Into frame` isn't a thing — `frame` is last paper. `Into` a `Pixels` State field.",
                    );
                    return None;
                }
                if !state.iter().any(|f| rust_name(&f.name) == n && is_pixels(&f.ty)) {
                    diags.error_once(
                        "gpu-into-src",
                        format!("`Into {n}` needs a `Pixels` State field."),
                    );
                    return None;
                }
                out.extend(flatten_gpu_cmds(body, Some(n), state, diags)?);
            }
            Stmt::Draw(DrawCmd::Clear { color }) => {
                out.push(GpuPass::Clear {
                    color: color.clone(),
                    target: target.clone(),
                });
            }
            Stmt::Draw(DrawCmd::Copy {
                src,
                args,
                mask,
                color_key,
                blend,
            }) => {
                let src_n = rust_name(src);
                if let Some(t) = &target {
                    if src_n == *t {
                        diags.error_once(
                            "gpu-copy-self",
                            format!("`Copy {src_n}` can't write into the same `Pixels` it's reading."),
                        );
                        return None;
                    }
                    if mask.as_ref().map(|m| rust_name(m)).as_deref() == Some(t.as_str()) {
                        diags.error_once(
                            "gpu-mask-self",
                            format!("`Using {t}` can't mask a copy that's writing `{t}`."),
                        );
                        return None;
                    }
                }
                out.push(GpuPass::Copy(CopyCmd {
                    src: src.clone(),
                    args: args.clone(),
                    mask: mask.clone(),
                    color_key: color_key.clone(),
                    blend: *blend,
                    target: target.clone(),
                }));
            }
            Stmt::For { .. } => {
                match strip_pixel_loops(std::slice::from_ref(s)) {
                    Some(k) => out.push(GpuPass::Kernel {
                        x: k.x,
                        y: k.y,
                        body: k.body,
                        target: target.clone(),
                    }),
                    None => {
                        diags.error_once(
                            "gpu-draw-shape",
                            "`Gpu Draw` pixel kernel should be `For y = 0 To height - 1` then \
                             `For x = 0 To width - 1` then `Set Pixel x, y, color`. Copy/Clear \
                             sit beside that loop, not inside it.",
                        );
                        return None;
                    }
                }
            }
            Stmt::Draw(DrawCmd::Pixel { .. }) => {
                diags.error_once(
                    "gpu-draw-shape",
                    "`Set Pixel` in `Gpu Draw` belongs inside `For y` / `For x`.",
                );
                return None;
            }
            Stmt::Draw(_) => {
                diags.error_once(
                    "gpu-draw-verb",
                    "`Fill` / `Stroke` / `Text` belong in CPU `Draw`, not `Gpu Draw`.",
                );
                return None;
            }
            _ => {
                diags.error_once(
                    "gpu-stmt",
                    "That statement isn't GPU-legal at the `Gpu Draw` top level. Use `Clear`, \
                     `Copy`, `Into spr`, or the `For y` / `For x` / `Set Pixel` kernel.",
                );
                return None;
            }
        }
    }
    Some(out)
}

fn wgsl_copy_fs(
    copy: &CopyCmd,
    state: &[StateField],
    uniforms: &HashSet<String>,
    diags: &mut Diagnostics,
) -> Option<String> {
    let src = rust_name(&copy.src);
    let src_wh = if src == "frame" {
        ("u.size.x".into(), "u.size.y".into())
    } else if state.iter().any(|f| rust_name(&f.name) == src && is_pixels(&f.ty)) {
        (format!("u.{src}_w"), format!("u.{src}_h"))
    } else {
        diags.error_once(
            "gpu-copy-src",
            format!(
                "`Copy {src}` needs a `Pixels` State field, or `frame` (the last GPU paper)."
            ),
        );
        return None;
    };
    if copy.mask.is_some() {
        let m = rust_name(copy.mask.as_ref().unwrap());
        if m != "frame"
            && !state
                .iter()
                .any(|f| rust_name(&f.name) == m && is_pixels(&f.ty))
        {
            diags.error_once(
                "gpu-copy-mask",
                format!("`Using {m}` needs a `Pixels` State field, or `frame`."),
            );
            return None;
        }
    }
    let a: Vec<String> = copy
        .args
        .iter()
        .map(|e| wgsl_expr(e, uniforms, diags))
        .collect::<Option<Vec<_>>>()?;
    let (dx, dy, dw, dh, sx, sy, sw, sh) = match a.len() {
        2 => (
            a[0].clone(),
            a[1].clone(),
            src_wh.0.clone(),
            src_wh.1.clone(),
            "0.0".into(),
            "0.0".into(),
            src_wh.0.clone(),
            src_wh.1.clone(),
        ),
        4 => (
            a[0].clone(),
            a[1].clone(),
            a[2].clone(),
            a[3].clone(),
            "0.0".into(),
            "0.0".into(),
            src_wh.0.clone(),
            src_wh.1.clone(),
        ),
        6 => (
            a[0].clone(),
            a[1].clone(),
            a[4].clone(),
            a[5].clone(),
            a[2].clone(),
            a[3].clone(),
            a[4].clone(),
            a[5].clone(),
        ),
        _ => unreachable!(),
    };
    let key = if let Some(c) = &copy.color_key {
        format!("let key = {};\n    let keyed = true;", wgsl_color(c, uniforms, diags)?)
    } else {
        "let key = vec4<f32>(0.0, 0.0, 0.0, 0.0);\n    let keyed = false;".into()
    };
    let mask_apply = if copy.mask.is_some() {
        "let m = textureSample(mask_tex, src_samp, uv);\n    \
         let cov = max(max(m.r, m.g), m.b);\n    \
         if cov < 0.04 { discard; }\n    \
         c.a = c.a * cov;\n    "
    } else {
        ""
    };
    Some(format!(
        "    let logical = pos.xy / u.scale;\n    \
         let dx = {dx};\n    let dy = {dy};\n    let dw = {dw};\n    let dh = {dh};\n    \
         if logical.x < dx || logical.y < dy || logical.x >= dx + dw || logical.y >= dy + dh {{ discard; }}\n    \
         let uv = vec2((logical.x - dx) / dw, (logical.y - dy) / dh);\n    \
         let src_uv = vec2(({sx} + uv.x * ({sw})) / ({src_w}), ({sy} + uv.y * ({sh})) / ({src_h}));\n    \
         var c = textureSample(src_tex, src_samp, src_uv);\n    \
         {key}\n    \
         if keyed && distance(c.rgb, key.rgb) < 0.04 {{ discard; }}\n    \
         {mask_apply}return c;\n",
        src_w = src_wh.0,
        src_h = src_wh.1,
    ))
}

struct PixelKernel {
    x: String,
    y: String,
    body: Vec<Stmt>,
}

/// Peel `For y … For x … body` (plus comments / line marks).
fn strip_pixel_loops(stmts: &[Stmt]) -> Option<PixelKernel> {
    let stmts = skip_noise(stmts);
    let Stmt::For { var: y, body: outer, .. } = stmts.first()? else {
        return None;
    };
    let inner_stmts = skip_noise(outer);
    let Stmt::For { var: x, body: inner, .. } = inner_stmts.first()? else {
        return None;
    };
    let body = skip_noise(inner);
    // At least one Set Pixel using those loop vars (case-insensitive).
    if !body.iter().any(|s| pixel_writes(s, x, y)) {
        return None;
    }
    Some(PixelKernel {
        x: x.clone(),
        y: y.clone(),
        body,
    })
}

fn skip_noise(stmts: &[Stmt]) -> Vec<Stmt> {
    stmts
        .iter()
        .filter(|s| !matches!(s, Stmt::Comment(_) | Stmt::LineMark(_)))
        .cloned()
        .collect()
}

fn pixel_writes(s: &Stmt, x: &str, y: &str) -> bool {
    match s {
        Stmt::Draw(DrawCmd::Pixel {
            x: xe,
            y: ye,
            ..
        }) => ident_is(xe, x) && ident_is(ye, y),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| b.iter().any(|s| pixel_writes(s, x, y)))
                || else_body
                    .as_ref()
                    .map_or(false, |b| b.iter().any(|s| pixel_writes(s, x, y)))
        }
        Stmt::DoLoop { body, .. } | Stmt::For { body, .. } => {
            body.iter().any(|s| pixel_writes(s, x, y))
        }
        _ => false,
    }
}

fn ident_is(e: &Expr, name: &str) -> bool {
    matches!(&e.kind, ExprKind::Ident(n) if n.eq_ignore_ascii_case(name))
}

fn wgsl_gpu_fn(f: &Function, uniforms: &HashSet<String>, diags: &mut Diagnostics) -> Option<String> {
    let name = rust_name(&f.name);
    let mut fn_uniforms = uniforms.clone();
    let mut copies = String::new();
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let r = rust_name(&p.name);
            fn_uniforms.remove(&r);
            copies.push_str(&format!("    var {r} = _{r};\n"));
            format!("_{r}: f32")
        })
        .collect();
    let ret = if f.ret.is_some() { " -> f32" } else { "" };
    let mut out = format!("fn {}({}){} {{\n", name, params.join(", "), ret);
    out.push_str(&copies);
    for s in skip_noise(&f.body) {
        out.push_str(&wgsl_stmt(&s, 1, &fn_uniforms, diags)?);
    }
    out.push_str("}\n");
    Some(out)
}

fn wgsl_stmt(s: &Stmt, indent: usize, uniforms: &HashSet<String>, diags: &mut Diagnostics) -> Option<String> {
    let pad = "    ".repeat(indent);
    match s {
        Stmt::Comment(_) | Stmt::LineMark(_) => Some(String::new()),
        Stmt::Dim { name, init, .. } => {
            let n = rust_name(name);
            match init {
                Some(e) => Some(format!("{pad}var {n} = {};\n", wgsl_expr(e, uniforms, diags)?)),
                None => Some(format!("{pad}var {n} = 0.0;\n")),
            }
        }
        Stmt::Assign { target, value, op } => {
            let t = wgsl_expr(target, uniforms, diags)?;
            let v = wgsl_expr(value, uniforms, diags)?;
            let rhs = match op {
                None => v,
                Some(BinOp::Add) => format!("{t} + {v}"),
                Some(BinOp::Sub) => format!("{t} - {v}"),
                Some(BinOp::Mul) => format!("{t} * {v}"),
                Some(BinOp::Div) => format!("{t} / {v}"),
                Some(_) => {
                    diags.error_once("gpu-compound", "That compound assignment isn't GPU-legal yet.");
                    return None;
                }
            };
            Some(format!("{pad}{t} = {rhs};\n"))
        }
        Stmt::Return(None) => Some(format!("{pad}return;\n")),
        Stmt::Return(Some(e)) => Some(format!("{pad}return {};\n", wgsl_expr(e, uniforms, diags)?)),
        Stmt::If {
            branches,
            else_body,
        } => {
            let mut out = String::new();
            for (i, (cond, body)) in branches.iter().enumerate() {
                let kw = if i == 0 { "if" } else { "} else if" };
                out.push_str(&format!("{pad}{kw} {} {{\n", wgsl_expr(cond, uniforms, diags)?));
                for s in body {
                    out.push_str(&wgsl_stmt(s, indent + 1, uniforms, diags)?);
                }
            }
            if let Some(body) = else_body {
                out.push_str(&format!("{pad}}} else {{\n"));
                for s in body {
                    out.push_str(&wgsl_stmt(s, indent + 1, uniforms, diags)?);
                }
            }
            out.push_str(&format!("{pad}}}\n"));
            Some(out)
        }
        Stmt::DoLoop { cond, body } => {
            let mut out = format!("{pad}loop {{\n");
            match cond {
                Some(DoCond::PreWhile(c)) => {
                    out.push_str(&format!(
                        "{pad}    if !({}) {{ break; }}\n",
                        wgsl_expr(c, uniforms, diags)?
                    ));
                }
                Some(DoCond::PreUntil(c)) => {
                    out.push_str(&format!(
                        "{pad}    if {} {{ break; }}\n",
                        wgsl_expr(c, uniforms, diags)?
                    ));
                }
                _ => {}
            }
            for s in body {
                out.push_str(&wgsl_stmt(s, indent + 1, uniforms, diags)?);
            }
            match cond {
                Some(DoCond::PostWhile(c)) => {
                    out.push_str(&format!(
                        "{pad}    if !({}) {{ break; }}\n",
                        wgsl_expr(c, uniforms, diags)?
                    ));
                }
                Some(DoCond::PostUntil(c)) => {
                    out.push_str(&format!(
                        "{pad}    if {} {{ break; }}\n",
                        wgsl_expr(c, uniforms, diags)?
                    ));
                }
                _ => {}
            }
            out.push_str(&format!("{pad}}}\n"));
            Some(out)
        }
        Stmt::Break => Some(format!("{pad}break;\n")),
        Stmt::Continue => Some(format!("{pad}continue;\n")),
        Stmt::Draw(DrawCmd::Pixel { color, .. }) => {
            Some(format!("{pad}col = {};\n", wgsl_color(color, uniforms, diags)?))
        }
        Stmt::Draw(_) => {
            diags.error_once(
                "gpu-draw-verb",
                "`Fill` / `Stroke` / `Text` belong in CPU `Draw`, not `Gpu Draw`.",
            );
            None
        }
        Stmt::For { var, from, to, step, body } => {
            let v = rust_name(var);
            let a = wgsl_expr(from, uniforms, diags)?;
            let b = wgsl_expr(to, uniforms, diags)?;
            let st = match step {
                Some(s) => wgsl_expr(s, uniforms, diags)?,
                None => "1.0".to_string(),
            };
            let mut out = format!("{pad}var {v} = {a};\n{pad}loop {{\n{pad}    if {v} > {b} {{ break; }}\n");
            for s in body {
                out.push_str(&wgsl_stmt(s, indent + 1, uniforms, diags)?);
            }
            out.push_str(&format!("{pad}    {v} = {v} + {st};\n{pad}}}\n"));
            Some(out)
        }
        other => {
            diags.error_once(
                "gpu-stmt",
                format!("That statement isn't GPU-legal yet ({:?}). Stick to Dim, If, For, Do, Return, Set Pixel.", std::mem::discriminant(other)),
            );
            None
        }
    }
}

fn wgsl_color(e: &Expr, uniforms: &HashSet<String>, diags: &mut Diagnostics) -> Option<String> {
    match &e.kind {
        ExprKind::Field(recv, name)
            if matches!(&recv.kind, ExprKind::Ident(n) if n.eq_ignore_ascii_case("Color")) =>
        {
            let (r, g, b) = named_rgb(name)?;
            Some(format!(
                "vec4<f32>({:.4}, {:.4}, {:.4}, 1.0)",
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0
            ))
        }
        ExprKind::Call { name, args } if name.eq_ignore_ascii_case("Color") && args.len() >= 3 => {
            let r = wgsl_expr(&args[0], uniforms, diags)?;
            let g = wgsl_expr(&args[1], uniforms, diags)?;
            let b = wgsl_expr(&args[2], uniforms, diags)?;
            Some(format!(
                "vec4<f32>(({r}) / 255.0, ({g}) / 255.0, ({b}) / 255.0, 1.0)"
            ))
        }
        _ => Some(format!("({})", wgsl_expr(e, uniforms, diags)?)),
    }
}

fn named_rgb(name: &str) -> Option<(u8, u8, u8)> {
    Some(match name.to_ascii_lowercase().as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "yellow" => (255, 255, 0),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "navy" => (0, 0, 128),
        "cyan" => (0, 255, 255),
        "magenta" => (255, 0, 255),
        _ => return None,
    })
}

fn wgsl_expr(e: &Expr, uniforms: &HashSet<String>, diags: &mut Diagnostics) -> Option<String> {
    match &e.kind {
        ExprKind::Int(n) => Some(format!("{n}.0")),
        ExprKind::Float(n) => {
            let s = format!("{n}");
            if s.contains('.') || s.contains('e') || s.contains('E') {
                Some(s)
            } else {
                Some(format!("{s}.0"))
            }
        }
        ExprKind::Bool(true) => Some("true".into()),
        ExprKind::Bool(false) => Some("false".into()),
        ExprKind::Ident(n) => {
            let r = rust_name(n);
            if r == "width" {
                Some("u.size.x".into())
            } else if r == "height" {
                Some("u.size.y".into())
            } else if uniforms.contains(&r) {
                Some(format!("u.{r}"))
            } else {
                Some(r)
            }
        }
        ExprKind::ConstRef(n) => {
            let leaf = n.rsplit("::").next().unwrap_or(n);
            Some(rust_name(leaf))
        }
        ExprKind::Not(inner) => Some(format!("!({})", wgsl_expr(inner, uniforms, diags)?)),
        ExprKind::Cast(inner, _) => wgsl_expr(inner, uniforms, diags),
        ExprKind::Deref(inner) | ExprKind::Ref(inner) | ExprKind::MutRef(inner) => {
            wgsl_expr(inner, uniforms, diags)
        }
        ExprKind::Try(inner) => wgsl_expr(inner, uniforms, diags),
        ExprKind::Binary { op, lhs, rhs } => {
            let a = wgsl_expr(lhs, uniforms, diags)?;
            let b = wgsl_expr(rhs, uniforms, diags)?;
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Mod => {
                    return Some(format!("(({a}) - ({b}) * floor(({a}) / ({b})))"));
                }
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::Le => "<=",
                BinOp::Ge => ">=",
                BinOp::And => "&&",
                BinOp::Or => "||",
                BinOp::Xor => "!=",
                BinOp::Pow => {
                    return Some(format!("pow(({a}), ({b}))"));
                }
                BinOp::Concat => {
                    diags.error_once("gpu-concat", "String `&` isn't GPU-legal.");
                    return None;
                }
            };
            Some(format!("(({a}) {o} ({b}))"))
        }
        ExprKind::Call { name, args } => wgsl_call(name, args, uniforms, diags),
        ExprKind::Field(recv, name) => {
            if let ExprKind::Ident(m) = &recv.kind {
                if m.eq_ignore_ascii_case("Color") {
                    return wgsl_color(e, uniforms, diags);
                }
            }
            Some(format!("{}.{}", wgsl_expr(recv, uniforms, diags)?, rust_name(name)))
        }
        ExprKind::MethodCall { recv, method, args } => {
            if let ExprKind::Ident(_) = &recv.kind {
                // `Lace.Escape(zr, zi)` — same-file GPU helper, drop the module name.
                return wgsl_call(method, args, uniforms, diags);
            }
            diags.error_once("gpu-method", "That method call isn't GPU-legal yet.");
            None
        }
        _ => {
            diags.error_once("gpu-expr", "That expression isn't GPU-legal yet.");
            None
        }
    }
}

fn wgsl_call(name: &str, args: &[Expr], uniforms: &HashSet<String>, diags: &mut Diagnostics) -> Option<String> {
    let n = name.to_ascii_lowercase();
    let a: Vec<String> = args
        .iter()
        .map(|e| wgsl_expr(e, uniforms, diags))
        .collect::<Option<Vec<_>>>()?;
    Some(match n.as_str() {
        "sin" => format!("sin({})", a[0]),
        "cos" => format!("cos({})", a[0]),
        "tan" => format!("tan({})", a[0]),
        "sqr" | "sqrt" => format!("sqrt({})", a[0]),
        "abs" => format!("abs({})", a[0]),
        "min" => format!("min({}, {})", a[0], a.get(1).unwrap_or(&a[0])),
        "max" => format!("max({}, {})", a[0], a.get(1).unwrap_or(&a[0])),
        "int" | "floor" => format!("floor({})", a[0]),
        "pow" => format!("pow({}, {})", a[0], a.get(1).unwrap_or(&a[0])),
        "color" => return wgsl_color(
            &ExprKind::Call {
                name: "Color".into(),
                args: args.to_vec(),
            }
            .synth(),
            uniforms,
            diags,
        ),
        _ => format!("{}({})", rust_name(name), a.join(", ")),
    })
}

/// Generated `Pixels` type — emitted before the Sketch `State` so a
/// `Dim spr As Pixels = Pixels.Of(18, 18)` field type-checks.
pub const PIXELS_TYPE: &str = r#"
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
struct Pixels {
    w: u32,
    h: u32,
}
impl Pixels {
    fn of(w: i64, h: i64) -> Self {
        Self { w: w.max(1) as u32, h: h.max(1) as u32 }
    }
}
"#;

fn extra_uniform_names(state: &[StateField]) -> Vec<String> {
    let mut extras = Vec::new();
    for f in state {
        if is_gpu_uniform(&f.ty) {
            extras.push(rust_name(&f.name));
        }
        if is_pixels(&f.ty) {
            let r = rust_name(&f.name);
            extras.push(format!("{r}_w"));
            extras.push(format!("{r}_h"));
        }
    }
    extras
}

fn tex_view(name: &str, prefix: &str) -> String {
    if rust_name(name) == "frame" || name.eq_ignore_ascii_case("frame") {
        format!("{prefix}frame_view")
    } else {
        format!("{prefix}view_{}", rust_name(name))
    }
}

fn tex_bg(name: &str) -> String {
    if rust_name(name) == "frame" || name.eq_ignore_ascii_case("frame") {
        "pipe.bg_frame".into()
    } else {
        format!("pipe.bg_{}", rust_name(name))
    }
}

fn dest_view(target: &Option<String>) -> String {
    match target {
        None => "pipe.paper_view".into(),
        Some(n) => format!("pipe.view_{}", rust_name(n)),
    }
}

fn dest_ubg(target: &Option<String>) -> String {
    match target {
        None => "pipe.ubg".into(),
        Some(n) => format!("pipe.ubg_{}", rust_name(n)),
    }
}

fn dest_first(target: &Option<String>) -> String {
    match target {
        None => "first_paper".into(),
        Some(n) => format!("first_{}", rust_name(n)),
    }
}

fn rust_runtime(
    sketch_name: &str,
    wgsl: &str,
    state: &[StateField],
    passes: &[RuntimePass],
) -> String {
    let extras = extra_uniform_names(state);
    let nfloats = 6 + extras.len();
    let nbytes = ((nfloats * 4 + 15) / 16) * 16;
    let struct_fields: String = extras.iter().map(|r| format!("    {r}: f32,\n")).collect();
    let (kernel_ty, prim_ty, pipe_ty) = (
        format!("{sketch_name}Kernel"),
        format!("{sketch_name}Prim"),
        format!("{sketch_name}Pipe"),
    );
    let kernel_def = if extras.is_empty() {
        format!("#[derive(Debug, Clone, Copy, Default)]\nstruct {kernel_ty};\n\n#[derive(Debug)]\nstruct {prim_ty};\n")
    } else {
        format!(
            "#[derive(Debug, Clone, Copy)]\nstruct {kernel_ty} {{\n{struct_fields}}}\n\n#[derive(Debug)]\nstruct {prim_ty} {{\n{struct_fields}}}\n"
        )
    };
    let prim_from = if extras.is_empty() {
        format!("{prim_ty}")
    } else {
        let fs: String = extras.iter().map(|r| format!("            {r}: self.{r},\n")).collect();
        format!("{prim_ty} {{\n{fs}        }}")
    };
    let extra_writes: String = extras.iter().map(|r| format!("            self.{r},\n")).collect();
    let pad_zeros = "0.0, ".repeat((nbytes / 4).saturating_sub(nfloats));
    let nwords = nbytes / 4;
    let wgsl_lit = format!("r#\"{}\"#", wgsl.replace("\"#", "\" #"));
    let pixel_fields: Vec<String> = state
        .iter()
        .filter(|f| is_pixels(&f.ty))
        .map(|f| rust_name(&f.name))
        .collect();
    let drawn_into: Vec<String> = {
        let mut v = Vec::new();
        for p in passes {
            let t = match p {
                RuntimePass::Clear { target: Some(t), .. }
                | RuntimePass::Kernel { target: Some(t), .. }
                | RuntimePass::Copy { target: Some(t), .. } => rust_name(t),
                _ => continue,
            };
            if !v.contains(&t) {
                v.push(t);
            }
        }
        v
    };
    let mut copy_fields = String::new();
    let mut copy_new = String::new();
    let mut copy_init = String::new();
    let mut mask_bg_new = String::new();
    let mut mask_bg_prep = String::new();
    for (i, p) in passes.iter().enumerate() {
        match p {
            RuntimePass::Copy { fs, blend, mask, src, .. } => {
                copy_fields.push_str(&format!("    copy_{i}: shader::wgpu::RenderPipeline,\n"));
                if let Some(m) = mask {
                    copy_fields.push_str(&format!("    bg_copy_{i}: shader::wgpu::BindGroup,\n"));
                    mask_bg_new.push_str(&format!(
                        "        let bg_copy_{i} = bind_tex_mask(device, &bgl1m, &{}, &samp, &{});\n",
                        tex_view(src, ""),
                        tex_view(m, ""),
                    ));
                    mask_bg_prep.push_str(&format!(
                        "        pipe.bg_copy_{i} = bind_tex_mask(device, &pipe.bgl1m, &{}, &pipe.samp, &{});\n",
                        tex_view(src, "pipe."),
                        tex_view(m, "pipe."),
                    ));
                }
                let blend_expr = match blend {
                    GpuBlend::Replace => "shader::wgpu::BlendState::REPLACE",
                    GpuBlend::Add => "shader::wgpu::BlendState { color: shader::wgpu::BlendComponent { src_factor: shader::wgpu::BlendFactor::One, dst_factor: shader::wgpu::BlendFactor::One, operation: shader::wgpu::BlendOperation::Add }, alpha: shader::wgpu::BlendComponent::REPLACE }",
                    GpuBlend::Multiply => "shader::wgpu::BlendState { color: shader::wgpu::BlendComponent { src_factor: shader::wgpu::BlendFactor::Dst, dst_factor: shader::wgpu::BlendFactor::Zero, operation: shader::wgpu::BlendOperation::Add }, alpha: shader::wgpu::BlendComponent::REPLACE }",
                };
                let layout = if mask.is_some() { "layout01m" } else { "layout01" };
                copy_new.push_str(&format!(
                    "        let copy_{i} = pipe_fs(\"{fs}\", &{layout}, {blend_expr});\n"
                ));
                copy_init.push_str(&format!("            copy_{i},\n"));
                if mask.is_some() {
                    copy_init.push_str(&format!("            bg_copy_{i},\n"));
                }
            }
            RuntimePass::Clear { fs, .. } => {
                copy_fields.push_str(&format!("    clear_{i}: shader::wgpu::RenderPipeline,\n"));
                copy_new.push_str(&format!(
                    "        let clear_{i} = pipe_fs(\"{fs}\", &layout0, shader::wgpu::BlendState::REPLACE);\n"
                ));
                copy_init.push_str(&format!("            clear_{i},\n"));
            }
            RuntimePass::Kernel { fs, .. } => {
                copy_fields.push_str(&format!("    kernel_{i}: shader::wgpu::RenderPipeline,\n"));
                copy_new.push_str(&format!(
                    "        let kernel_{i} = pipe_fs(\"{fs}\", &layout0, shader::wgpu::BlendState::REPLACE);\n"
                ));
                copy_init.push_str(&format!("            kernel_{i},\n"));
            }
        }
    }
    let pix_tex_fields: String = pixel_fields
        .iter()
        .map(|r| {
            format!(
                "    tex_{r}: shader::wgpu::Texture,\n    view_{r}: shader::wgpu::TextureView,\n    \
                 bg_{r}: shader::wgpu::BindGroup,\n    uniforms_{r}: shader::wgpu::Buffer,\n    ubg_{r}: shader::wgpu::BindGroup,\n"
            )
        })
        .collect();
    let pix_tex_new: String = pixel_fields
        .iter()
        .map(|r| {
            format!(
                "        let (tex_{r}, view_{r}) = make_solid(device, format, 1, 1);\n        \
                 let bg_{r} = bind_tex(&bgl1, &view_{r}, &samp, device);\n        \
                 let uniforms_{r} = device.create_buffer(&shader::wgpu::BufferDescriptor {{\n            \
                 label: Some(\"vbr gpu uniforms {r}\"),\n            size: {nbytes},\n            \
                 usage: shader::wgpu::BufferUsages::UNIFORM | shader::wgpu::BufferUsages::COPY_DST,\n            \
                 mapped_at_creation: false,\n        }});\n        \
                 let ubg_{r} = device.create_bind_group(&shader::wgpu::BindGroupDescriptor {{\n            \
                 label: Some(\"vbr gpu ubg {r}\"),\n            layout: &bgl0,\n            \
                 entries: &[shader::wgpu::BindGroupEntry {{\n                binding: 0,\n                \
                 resource: uniforms_{r}.as_entire_binding(),\n            }}],\n        }});\n"
            )
        })
        .collect();
    let pix_tex_new = pix_tex_new + &mask_bg_new;
    let pix_tex_init: String = pixel_fields
        .iter()
        .map(|r| format!("            tex_{r}, view_{r}, bg_{r}, uniforms_{r}, ubg_{r},\n"))
        .collect();
    let mut first_inits = String::from("        let mut first_paper = true;\n");
    for r in &drawn_into {
        first_inits.push_str(&format!("        let mut first_{r} = true;\n"));
    }
    let mut render_passes = first_inits;
    for (i, p) in passes.iter().enumerate() {
        let (pipe_field, groups, target) = match p {
            RuntimePass::Kernel { target, .. } => (
                format!("pipe.kernel_{i}"),
                String::new(),
                target,
            ),
            RuntimePass::Clear { target, .. } => (
                format!("pipe.clear_{i}"),
                String::new(),
                target,
            ),
            RuntimePass::Copy { src, mask, target, .. } => {
                let g = if mask.is_some() {
                    format!("            pass.set_bind_group(1, &pipe.bg_copy_{i}, &[]);\n")
                } else {
                    format!(
                        "            pass.set_bind_group(1, &{}, &[]);\n",
                        tex_bg(src)
                    )
                };
                (format!("pipe.copy_{i}"), g, target)
            }
        };
        let view = dest_view(target);
        let ubg = dest_ubg(target);
        let first = dest_first(target);
        let load = format!(
            "if {first} {{ shader::wgpu::LoadOp::Clear(shader::wgpu::Color::TRANSPARENT) }} else {{ shader::wgpu::LoadOp::Load }}"
        );
        render_passes.push_str(&format!(
            "        {{\n            let mut pass = begin(encoder, &{view}, {load});\n            \
             pass.set_pipeline(&{pipe_field});\n            pass.set_bind_group(0, &{ubg}, &[]);\n{groups}            \
             pass.draw(0..3, 0..1);\n            {first} = false;\n        }}\n"
        ));
    }
    for r in std::iter::once("paper".to_string()).chain(drawn_into.iter().cloned()) {
        render_passes.push_str(&format!("        let _ = first_{r};\n"));
    }
    let resize_spr: String = pixel_fields
        .iter()
        .map(|r| {
            let fill = if drawn_into.contains(r) {
                "[0, 0, 0, 0]"
            } else {
                "[255, 255, 255, 255]"
            };
            format!(
                "        let want_{r} = (self.{r}_w.max(1.0) as u32, self.{r}_h.max(1.0) as u32);\n        \
                 if pipe.tex_{r}.size().width != want_{r}.0 || pipe.tex_{r}.size().height != want_{r}.1 || pipe.format != format {{\n            \
                 let (t, v) = make_solid(device, format, want_{r}.0, want_{r}.1);\n            \
                 pipe.bg_{r} = bind_tex(&pipe.bgl1, &v, &pipe.samp, device);\n            \
                 pipe.tex_{r} = t; pipe.view_{r} = v;\n            \
                 fill_solid(queue, &pipe.tex_{r}, want_{r}.0, want_{r}.1, {fill});\n        }}\n"
            )
        })
        .collect();
    let resize_spr = resize_spr + &mask_bg_prep;
    let pix_uniform_writes: String = pixel_fields
        .iter()
        .map(|r| {
            format!(
                "        let data_{r}: [f32; {nwords}] = [\n            0.0, 0.0, 1.0, 0.0,\n            \
                 self.{r}_w, self.{r}_h,\n{extra_writes}            {pad_zeros}\n        ];\n        \
                 let bytes_{r} = unsafe {{\n            std::slice::from_raw_parts(data_{r}.as_ptr() as *const u8, std::mem::size_of_val(&data_{r}))\n        }};\n        \
                 queue.write_buffer(&pipe.uniforms_{r}, 0, bytes_{r});\n"
            )
        })
        .collect();
    format!(
        r#"
use iced::widget::shader;

{kernel_def}
impl<Message> shader::Program<Message> for {kernel_ty} {{
    type State = ();
    type Primitive = {prim_ty};
    fn draw(
        &self,
        _state: &Self::State,
        _cursor: iced::mouse::Cursor,
        _bounds: iced::Rectangle,
    ) -> Self::Primitive {{
        {prim_from}
    }}
}}

#[allow(dead_code)]
struct {pipe_ty} {{
    format: shader::wgpu::TextureFormat,
    bgl1: shader::wgpu::BindGroupLayout,
    bgl1m: shader::wgpu::BindGroupLayout,
    ubg: shader::wgpu::BindGroup,
    uniforms: shader::wgpu::Buffer,
    samp: shader::wgpu::Sampler,
    blit: shader::wgpu::RenderPipeline,
{copy_fields}    paper: shader::wgpu::Texture,
    paper_view: shader::wgpu::TextureView,
    frame: shader::wgpu::Texture,
    frame_view: shader::wgpu::TextureView,
    bg_frame: shader::wgpu::BindGroup,
    bg_paper: shader::wgpu::BindGroup,
{pix_tex_fields}}}
"#,
        kernel_def = kernel_def,
        kernel_ty = kernel_ty,
        prim_ty = prim_ty,
        pipe_ty = pipe_ty,
        prim_from = prim_from,
        copy_fields = copy_fields,
        pix_tex_fields = pix_tex_fields,
    ) + &format!(
        r#"
impl shader::Primitive for {prim_ty} {{
    fn prepare(
        &self,
        device: &shader::wgpu::Device,
        queue: &shader::wgpu::Queue,
        format: shader::wgpu::TextureFormat,
        storage: &mut shader::Storage,
        bounds: &iced::Rectangle,
        viewport: &shader::Viewport,
    ) {{
        if !storage.has::<{pipe_ty}>() {{
            storage.store({pipe_ty}::new(device, format));
        }}
        let pipe = storage.get_mut::<{pipe_ty}>().unwrap();
        let scale = viewport.scale_factor() as f32;
        let pw = (bounds.width * scale).max(1.0).round() as u32;
        let ph = (bounds.height * scale).max(1.0).round() as u32;
        if pipe.paper.size().width != pw || pipe.paper.size().height != ph || pipe.format != format {{
            let (paper, paper_view) = make_target(device, format, pw, ph);
            let (frame, frame_view) = make_target(device, format, pw, ph);
            pipe.bg_frame = bind_tex(&pipe.bgl1, &frame_view, &pipe.samp, device);
            pipe.bg_paper = bind_tex(&pipe.bgl1, &paper_view, &pipe.samp, device);
            pipe.paper = paper;
            pipe.paper_view = paper_view;
            pipe.frame = frame;
            pipe.frame_view = frame_view;
            pipe.format = format;
            fill_solid(queue, &pipe.frame, pw, ph, [0, 0, 0, 0]);
        }}
{resize_spr}        let data: [f32; {nwords}] = [
            bounds.x * scale,
            bounds.y * scale,
            scale,
            0.0,
            bounds.width,
            bounds.height,
{extra_writes}            {pad_zeros}
        ];
        let bytes = unsafe {{
            std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(&data))
        }};
        queue.write_buffer(&pipe.uniforms, 0, bytes);
{pix_uniform_writes}    }}

    fn render(
        &self,
        encoder: &mut shader::wgpu::CommandEncoder,
        storage: &shader::Storage,
        target: &shader::wgpu::TextureView,
        clip_bounds: &iced::Rectangle<u32>,
    ) {{
        let pipe = storage.get::<{pipe_ty}>().unwrap();
{render_passes}        {{
            let mut pass = begin(encoder, target, shader::wgpu::LoadOp::Load);
            pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width.max(1), clip_bounds.height.max(1));
            pass.set_pipeline(&pipe.blit);
            pass.set_bind_group(0, &pipe.ubg, &[]);
            pass.set_bind_group(1, &pipe.bg_paper, &[]);
            pass.draw(0..3, 0..1);
        }}
        encoder.copy_texture_to_texture(
            pipe.paper.as_image_copy(),
            pipe.frame.as_image_copy(),
            pipe.paper.size(),
        );
    }}
}}
"#,
        prim_ty = prim_ty,
        pipe_ty = pipe_ty,
        extra_writes = extra_writes,
        pad_zeros = pad_zeros,
        nwords = nbytes / 4,
        resize_spr = resize_spr,
        pix_uniform_writes = pix_uniform_writes,
        render_passes = render_passes,
    ) + &gpu_helpers(
        nbytes as u64,
        &wgsl_lit,
        &pipe_ty,
        &copy_new,
        &copy_init,
        &pix_tex_new,
        &pix_tex_init,
    )
}

fn gpu_helpers(
    nbytes: u64,
    wgsl_lit: &str,
    pipe_ty: &str,
    copy_new: &str,
    copy_init: &str,
    pix_tex_new: &str,
    pix_tex_init: &str,
) -> String {
    format!(
        r#"
fn begin<'a>(
    encoder: &'a mut shader::wgpu::CommandEncoder,
    view: &'a shader::wgpu::TextureView,
    load: shader::wgpu::LoadOp<shader::wgpu::Color>,
) -> shader::wgpu::RenderPass<'a> {{
    encoder.begin_render_pass(&shader::wgpu::RenderPassDescriptor {{
        label: Some("vbr gpu pass"),
        color_attachments: &[Some(shader::wgpu::RenderPassColorAttachment {{
            view,
            resolve_target: None,
            ops: shader::wgpu::Operations {{
                load,
                store: shader::wgpu::StoreOp::Store,
            }},
        }})],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    }})
}}

fn bind_tex(
    bgl1: &shader::wgpu::BindGroupLayout,
    view: &shader::wgpu::TextureView,
    samp: &shader::wgpu::Sampler,
    device: &shader::wgpu::Device,
) -> shader::wgpu::BindGroup {{
    device.create_bind_group(&shader::wgpu::BindGroupDescriptor {{
        label: Some("vbr gpu tex"),
        layout: bgl1,
        entries: &[
            shader::wgpu::BindGroupEntry {{
                binding: 0,
                resource: shader::wgpu::BindingResource::TextureView(view),
            }},
            shader::wgpu::BindGroupEntry {{
                binding: 1,
                resource: shader::wgpu::BindingResource::Sampler(samp),
            }},
        ],
    }})
}}

#[allow(dead_code)]
fn bind_tex_mask(
    device: &shader::wgpu::Device,
    bgl1m: &shader::wgpu::BindGroupLayout,
    src: &shader::wgpu::TextureView,
    samp: &shader::wgpu::Sampler,
    mask: &shader::wgpu::TextureView,
) -> shader::wgpu::BindGroup {{
    device.create_bind_group(&shader::wgpu::BindGroupDescriptor {{
        label: Some("vbr gpu tex mask"),
        layout: bgl1m,
        entries: &[
            shader::wgpu::BindGroupEntry {{
                binding: 0,
                resource: shader::wgpu::BindingResource::TextureView(src),
            }},
            shader::wgpu::BindGroupEntry {{
                binding: 1,
                resource: shader::wgpu::BindingResource::Sampler(samp),
            }},
            shader::wgpu::BindGroupEntry {{
                binding: 2,
                resource: shader::wgpu::BindingResource::TextureView(mask),
            }},
        ],
    }})
}}

fn make_target(
    device: &shader::wgpu::Device,
    format: shader::wgpu::TextureFormat,
    w: u32,
    h: u32,
) -> (shader::wgpu::Texture, shader::wgpu::TextureView) {{
    let tex = device.create_texture(&shader::wgpu::TextureDescriptor {{
        label: Some("vbr gpu target"),
        size: shader::wgpu::Extent3d {{ width: w.max(1), height: h.max(1), depth_or_array_layers: 1 }},
        mip_level_count: 1,
        sample_count: 1,
        dimension: shader::wgpu::TextureDimension::D2,
        format,
        usage: shader::wgpu::TextureUsages::RENDER_ATTACHMENT
            | shader::wgpu::TextureUsages::TEXTURE_BINDING
            | shader::wgpu::TextureUsages::COPY_SRC
            | shader::wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    }});
    let view = tex.create_view(&shader::wgpu::TextureViewDescriptor::default());
    (tex, view)
}}

#[allow(dead_code)]
fn make_solid(
    device: &shader::wgpu::Device,
    format: shader::wgpu::TextureFormat,
    w: u32,
    h: u32,
) -> (shader::wgpu::Texture, shader::wgpu::TextureView) {{
    let w = w.max(1);
    let h = h.max(1);
    let tex = device.create_texture(&shader::wgpu::TextureDescriptor {{
        label: Some("vbr gpu pixels"),
        size: shader::wgpu::Extent3d {{ width: w, height: h, depth_or_array_layers: 1 }},
        mip_level_count: 1,
        sample_count: 1,
        dimension: shader::wgpu::TextureDimension::D2,
        format,
        usage: shader::wgpu::TextureUsages::RENDER_ATTACHMENT
            | shader::wgpu::TextureUsages::TEXTURE_BINDING
            | shader::wgpu::TextureUsages::COPY_SRC
            | shader::wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    }});
    let view = tex.create_view(&shader::wgpu::TextureViewDescriptor::default());
    (tex, view)
}}

fn fill_solid(
    queue: &shader::wgpu::Queue,
    tex: &shader::wgpu::Texture,
    w: u32,
    h: u32,
    rgba: [u8; 4],
) {{
    let w = w.max(1);
    let h = h.max(1);
    let row = ((w * 4 + 255) / 256) * 256;
    let mut data = vec![0u8; (row * h) as usize];
    if rgba != [0, 0, 0, 0] {{
        for y in 0..h {{
            for x in 0..w {{
                let i = (y * row + x * 4) as usize;
                data[i..i + 4].copy_from_slice(&rgba);
            }}
        }}
    }}
    queue.write_texture(
        tex.as_image_copy(),
        &data,
        shader::wgpu::ImageDataLayout {{
            offset: 0,
            bytes_per_row: Some(row),
            rows_per_image: Some(h),
        }},
        shader::wgpu::Extent3d {{ width: w, height: h, depth_or_array_layers: 1 }},
    );
}}

impl {pipe_ty} {{
    fn new(device: &shader::wgpu::Device, format: shader::wgpu::TextureFormat) -> Self {{
        let shader = device.create_shader_module(shader::wgpu::ShaderModuleDescriptor {{
            label: Some("vbr gpu draw"),
            source: shader::wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed({wgsl_lit})),
        }});
        let bgl0 = device.create_bind_group_layout(&shader::wgpu::BindGroupLayoutDescriptor {{
            label: Some("vbr gpu uniforms"),
            entries: &[shader::wgpu::BindGroupLayoutEntry {{
                binding: 0,
                visibility: shader::wgpu::ShaderStages::FRAGMENT,
                ty: shader::wgpu::BindingType::Buffer {{
                    ty: shader::wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                }},
                count: None,
            }}],
        }});
        let bgl1 = device.create_bind_group_layout(&shader::wgpu::BindGroupLayoutDescriptor {{
            label: Some("vbr gpu tex"),
            entries: &[
                shader::wgpu::BindGroupLayoutEntry {{
                    binding: 0,
                    visibility: shader::wgpu::ShaderStages::FRAGMENT,
                    ty: shader::wgpu::BindingType::Texture {{
                        sample_type: shader::wgpu::TextureSampleType::Float {{ filterable: true }},
                        view_dimension: shader::wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    }},
                    count: None,
                }},
                shader::wgpu::BindGroupLayoutEntry {{
                    binding: 1,
                    visibility: shader::wgpu::ShaderStages::FRAGMENT,
                    ty: shader::wgpu::BindingType::Sampler(shader::wgpu::SamplerBindingType::Filtering),
                    count: None,
                }},
            ],
        }});
        let bgl1m = device.create_bind_group_layout(&shader::wgpu::BindGroupLayoutDescriptor {{
            label: Some("vbr gpu tex mask"),
            entries: &[
                shader::wgpu::BindGroupLayoutEntry {{
                    binding: 0,
                    visibility: shader::wgpu::ShaderStages::FRAGMENT,
                    ty: shader::wgpu::BindingType::Texture {{
                        sample_type: shader::wgpu::TextureSampleType::Float {{ filterable: true }},
                        view_dimension: shader::wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    }},
                    count: None,
                }},
                shader::wgpu::BindGroupLayoutEntry {{
                    binding: 1,
                    visibility: shader::wgpu::ShaderStages::FRAGMENT,
                    ty: shader::wgpu::BindingType::Sampler(shader::wgpu::SamplerBindingType::Filtering),
                    count: None,
                }},
                shader::wgpu::BindGroupLayoutEntry {{
                    binding: 2,
                    visibility: shader::wgpu::ShaderStages::FRAGMENT,
                    ty: shader::wgpu::BindingType::Texture {{
                        sample_type: shader::wgpu::TextureSampleType::Float {{ filterable: true }},
                        view_dimension: shader::wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    }},
                    count: None,
                }},
            ],
        }});
        let uniforms = device.create_buffer(&shader::wgpu::BufferDescriptor {{
            label: Some("vbr gpu uniforms"),
            size: {nbytes},
            usage: shader::wgpu::BufferUsages::UNIFORM | shader::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }});
        let ubg = device.create_bind_group(&shader::wgpu::BindGroupDescriptor {{
            label: Some("vbr gpu ubg"),
            layout: &bgl0,
            entries: &[shader::wgpu::BindGroupEntry {{
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }}],
        }});
        let samp = device.create_sampler(&shader::wgpu::SamplerDescriptor {{
            label: Some("vbr gpu samp"),
            mag_filter: shader::wgpu::FilterMode::Linear,
            min_filter: shader::wgpu::FilterMode::Linear,
            mipmap_filter: shader::wgpu::FilterMode::Nearest,
            ..Default::default()
        }});
        let layout0 = device.create_pipeline_layout(&shader::wgpu::PipelineLayoutDescriptor {{
            label: Some("vbr gpu layout0"),
            bind_group_layouts: &[&bgl0],
            push_constant_ranges: &[],
        }});
        let layout01 = device.create_pipeline_layout(&shader::wgpu::PipelineLayoutDescriptor {{
            label: Some("vbr gpu layout01"),
            bind_group_layouts: &[&bgl0, &bgl1],
            push_constant_ranges: &[],
        }});
        let layout01m = device.create_pipeline_layout(&shader::wgpu::PipelineLayoutDescriptor {{
            label: Some("vbr gpu layout01m"),
            bind_group_layouts: &[&bgl0, &bgl1m],
            push_constant_ranges: &[],
        }});
        let _ = &layout01m;
        let pipe_fs = |entry: &str, layout: &shader::wgpu::PipelineLayout, blend: shader::wgpu::BlendState| {{
            device.create_render_pipeline(&shader::wgpu::RenderPipelineDescriptor {{
                label: Some("vbr gpu pipeline"),
                layout: Some(layout),
                vertex: shader::wgpu::VertexState {{
                    module: &shader,
                    entry_point: "vs",
                    buffers: &[],
                }},
                fragment: Some(shader::wgpu::FragmentState {{
                    module: &shader,
                    entry_point: entry,
                    targets: &[Some(shader::wgpu::ColorTargetState {{
                        format,
                        blend: Some(blend),
                        write_mask: shader::wgpu::ColorWrites::ALL,
                    }})],
                }}),
                primitive: shader::wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: shader::wgpu::MultisampleState::default(),
                multiview: None,
            }})
        }};
        let blit = pipe_fs("fs_blit", &layout01, shader::wgpu::BlendState::REPLACE);
{copy_new}        let (paper, paper_view) = make_target(device, format, 1, 1);
        let (frame, frame_view) = make_target(device, format, 1, 1);
        let bg_frame = bind_tex(&bgl1, &frame_view, &samp, device);
        let bg_paper = bind_tex(&bgl1, &paper_view, &samp, device);
{pix_tex_new}        Self {{
            format,
            bgl1,
            bgl1m,
            ubg,
            uniforms,
            samp,
            blit,
{copy_init}            paper,
            paper_view,
            frame,
            frame_view,
            bg_frame,
            bg_paper,
{pix_tex_init}        }}
    }}
}}
"#,
        nbytes = nbytes,
        wgsl_lit = wgsl_lit,
        pipe_ty = pipe_ty,
        copy_new = copy_new,
        pix_tex_new = pix_tex_new,
        copy_init = copy_init,
        pix_tex_init = pix_tex_init,
    )
}

/// `PaneKernel { t: state.t as f32 }` (or `PaneKernel` with no uniforms).
pub fn kernel_new_expr(sketch_name: &str, state: &[StateField]) -> String {
    let extras = extra_uniform_names(state);
    if extras.is_empty() {
        return format!("{sketch_name}Kernel");
    }
    let fields: Vec<String> = state
        .iter()
        .flat_map(|f| {
            let r = rust_name(&f.name);
            let mut v = Vec::new();
            if is_gpu_uniform(&f.ty) {
                v.push(format!("{r}: state.{r} as f32"));
            }
            if is_pixels(&f.ty) {
                v.push(format!("{r}_w: state.{r}.w as f32"));
                v.push(format!("{r}_h: state.{r}.h as f32"));
            }
            v
        })
        .collect();
    format!("{sketch_name}Kernel {{ {} }}", fields.join(", "))
}
