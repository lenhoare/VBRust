//! `Gpu Draw` / `Gpu Function` → WGSL fragment shader + an iced `Shader` widget.
//!
//! V1 is a pixel kernel: nested `For y` / `For x` / `Set Pixel x, y, color`
//! becomes one fragment. Helpers marked `Gpu Function` are emitted as WGSL
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
    let kernel = match strip_pixel_loops(gpu_draw) {
        Some(k) => k,
        None => {
            diags.error_once(
                "gpu-draw-shape",
                "`Gpu Draw` should be `For y = 0 To height - 1` then `For x = 0 To width - 1` \
                 then `Set Pixel x, y, color` — the pixel kernel. Copy/masks come later.",
            );
            return None;
        }
    };
    for f in state {
        let r = rust_name(&f.name);
        if is_gpu_uniform(&f.ty) && RESERVED_UNIFORM.contains(&r.as_str()) {
            diags.error_once(
                "gpu-uniform-name",
                format!(
                    "State field `{r}` collides with a Gpu Draw uniform name. Pick another name."
                ),
            );
            return None;
        }
    }
    let mut wgsl = String::new();
    wgsl.push_str("struct Uniforms {\n    origin: vec2<f32>,\n    scale: f32,\n    _pad: f32,\n    size: vec2<f32>,\n");
    for f in state {
        if is_gpu_uniform(&f.ty) {
            wgsl.push_str(&format!("    {}: f32,\n", rust_name(&f.name)));
        }
    }
    wgsl.push_str("}\n@group(0) @binding(0) var<uniform> u: Uniforms;\n\n");
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

    let uniforms: HashSet<String> = state
        .iter()
        .filter(|f| is_gpu_uniform(&f.ty))
        .map(|f| rust_name(&f.name))
        .collect();
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
    let x = rust_name(&kernel.x);
    let y = rust_name(&kernel.y);
    wgsl.push_str(&format!(
        "@fragment\nfn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{\n    \
         let logical = (pos.xy - u.origin) / u.scale;\n    \
         let {x} = logical.x;\n    \
         let {y} = logical.y;\n    \
         if {x} < 0.0 || {y} < 0.0 || {x} >= u.size.x || {y} >= u.size.y {{ discard; }}\n    \
         var col = vec4<f32>(0.0, 0.0, 0.0, 1.0);\n"
    ));
    for s in &kernel.body {
        match wgsl_stmt(s, 1, &uniforms, diags) {
            Some(line) => wgsl.push_str(&line),
            None => return None,
        }
    }
    wgsl.push_str("    return col;\n}\n");

    Some(rust_runtime(sketch_name, &wgsl, state))
}

pub(crate) fn is_gpu_uniform(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(t) if is_gpu_numeric_type(t))
}

fn is_gpu_numeric_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Integer | Type::Long | Type::LongLong | Type::Single | Type::Double | Type::Byte
    )
}

const RESERVED_UNIFORM: &[&str] = &["origin", "scale", "size", "_pad", "u", "col", "pos", "logical"];

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
                BinOp::Mod => "%",
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

fn rust_runtime(sketch_name: &str, wgsl: &str, state: &[StateField]) -> String {
    let extras: Vec<String> = state
        .iter()
        .filter(|f| is_gpu_uniform(&f.ty))
        .map(|f| rust_name(&f.name))
        .collect();
    let nfloats = 6 + extras.len();
    let nbytes = ((nfloats * 4 + 15) / 16) * 16;
    let struct_fields: String = extras
        .iter()
        .map(|r| format!("    {r}: f32,\n"))
        .collect();
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
        let fs: String = extras
            .iter()
            .map(|r| format!("            {r}: self.{r},\n"))
            .collect();
        format!("{prim_ty} {{\n{fs}        }}")
    };
    let extra_writes: String = extras.iter().map(|r| format!("            self.{r},\n")).collect();
    let pad_zeros = "0.0, ".repeat((nbytes / 4).saturating_sub(nfloats));
    let wgsl_lit = format!("r#\"{}\"#", wgsl.replace("\"#", "\" #"));
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

struct {pipe_ty} {{
    pipeline: shader::wgpu::RenderPipeline,
    bind_group: shader::wgpu::BindGroup,
    uniforms: shader::wgpu::Buffer,
}}

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
        let data: [f32; {nwords}] = [
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
        let _ = device;
    }}

    fn render(
        &self,
        encoder: &mut shader::wgpu::CommandEncoder,
        storage: &shader::Storage,
        target: &shader::wgpu::TextureView,
        clip_bounds: &iced::Rectangle<u32>,
    ) {{
        let pipe = storage.get::<{pipe_ty}>().unwrap();
        let mut pass = encoder.begin_render_pass(&shader::wgpu::RenderPassDescriptor {{
            label: Some("vbr gpu draw"),
            color_attachments: &[Some(shader::wgpu::RenderPassColorAttachment {{
                view: target,
                resolve_target: None,
                ops: shader::wgpu::Operations {{
                    load: shader::wgpu::LoadOp::Load,
                    store: shader::wgpu::StoreOp::Store,
                }},
            }})],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        }});
        pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width.max(1), clip_bounds.height.max(1));
        pass.set_pipeline(&pipe.pipeline);
        pass.set_bind_group(0, &pipe.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }}
}}

impl {pipe_ty} {{
    fn new(device: &shader::wgpu::Device, format: shader::wgpu::TextureFormat) -> Self {{
        let shader = device.create_shader_module(shader::wgpu::ShaderModuleDescriptor {{
            label: Some("vbr gpu draw"),
            source: shader::wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed({wgsl_lit})),
        }});
        let bgl = device.create_bind_group_layout(&shader::wgpu::BindGroupLayoutDescriptor {{
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
        let uniforms = device.create_buffer(&shader::wgpu::BufferDescriptor {{
            label: Some("vbr gpu uniforms"),
            size: {nbytes},
            usage: shader::wgpu::BufferUsages::UNIFORM | shader::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }});
        let bind_group = device.create_bind_group(&shader::wgpu::BindGroupDescriptor {{
            label: Some("vbr gpu bind"),
            layout: &bgl,
            entries: &[shader::wgpu::BindGroupEntry {{
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }}],
        }});
        let layout = device.create_pipeline_layout(&shader::wgpu::PipelineLayoutDescriptor {{
            label: Some("vbr gpu layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        }});
        let pipeline = device.create_render_pipeline(&shader::wgpu::RenderPipelineDescriptor {{
            label: Some("vbr gpu pipeline"),
            layout: Some(&layout),
            vertex: shader::wgpu::VertexState {{
                module: &shader,
                entry_point: "vs",
                buffers: &[],
            }},
            fragment: Some(shader::wgpu::FragmentState {{
                module: &shader,
                entry_point: "fs",
                targets: &[Some(shader::wgpu::ColorTargetState {{
                    format,
                    blend: Some(shader::wgpu::BlendState::REPLACE),
                    write_mask: shader::wgpu::ColorWrites::ALL,
                }})],
            }}),
            primitive: shader::wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: shader::wgpu::MultisampleState::default(),
            multiview: None,
        }});
        Self {{ pipeline, bind_group, uniforms }}
    }}
}}
"#,
        kernel_def = kernel_def,
        kernel_ty = kernel_ty,
        prim_ty = prim_ty,
        pipe_ty = pipe_ty,
        prim_from = prim_from,
        extra_writes = extra_writes,
        pad_zeros = pad_zeros,
        nwords = nbytes / 4,
        nbytes = nbytes,
        wgsl_lit = wgsl_lit,
    )
}

/// `PaneKernel { t: state.t as f32 }` (or `PaneKernel` with no uniforms).
pub fn kernel_new_expr(sketch_name: &str, state: &[StateField]) -> String {
    let extras: Vec<String> = state
        .iter()
        .filter(|f| is_gpu_uniform(&f.ty))
        .map(|f| {
            let r = rust_name(&f.name);
            format!("{r}: state.{r} as f32")
        })
        .collect();
    if extras.is_empty() {
        format!("{sketch_name}Kernel")
    } else {
        format!("{sketch_name}Kernel {{ {} }}", extras.join(", "))
    }
}

