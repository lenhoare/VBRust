//! In-process `Screen` host for the phone.
//!
//! Desktop TUI compiles a `Screen` to ratatui and drives it with Tab / Space /
//! Enter. The phone has no rustc and TinyCC cannot run that Rust. This module
//! interprets the same AST (State / View / Events) and dumps a JSON widget tree
//! the WebView paints as a clickable Turbo Vision surface.

use serde::Serialize;
use serde_json::{json, Value as Json};
use std::collections::HashMap;
use std::sync::Mutex;
use vbr::ast::*;
use vbr::diagnostics::Diagnostics;
use vbr::lexer;
use vbr::parser;

static SESSION: Mutex<Option<Host>> = Mutex::new(None);

#[derive(Clone, Debug)]
enum Val {
    Null,
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Enum { ty: String, variant: String },
    List(Vec<Val>),
}

impl Val {
    fn as_bool(&self) -> bool {
        match self {
            Val::Bool(b) => *b,
            Val::Int(n) => *n != 0,
            Val::Float(n) => *n != 0.0,
            Val::Str(s) => !s.is_empty(),
            Val::List(xs) => !xs.is_empty(),
            Val::Null => false,
            Val::Enum { .. } => true,
        }
    }

    fn as_int(&self) -> i64 {
        match self {
            Val::Int(n) => *n,
            Val::Float(n) => *n as i64,
            Val::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            Val::Str(s) => s.parse().unwrap_or(0),
            _ => 0,
        }
    }

    fn as_float(&self) -> f64 {
        match self {
            Val::Float(n) => *n,
            Val::Int(n) => *n as f64,
            Val::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Val::Str(s) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    fn stringify(&self) -> String {
        match self {
            Val::Null => String::new(),
            Val::Int(n) => n.to_string(),
            Val::Float(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Val::Bool(b) => {
                if *b {
                    "True".into()
                } else {
                    "False".into()
                }
            }
            Val::Str(s) => s.clone(),
            Val::Enum { ty, variant } => {
                let _ = ty;
                variant.clone()
            }
            Val::List(xs) => xs
                .iter()
                .map(|v| v.stringify())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    fn is_floatish(&self) -> bool {
        matches!(self, Val::Float(_))
    }
}

enum Flow {
    Next,
    Return(Val),
    Break,
    Continue,
}

struct Host {
    program: Program,
    screen_idx: usize,
    state: HashMap<String, Val>,
    field_types: HashMap<String, DeclType>,
    list_sel: HashMap<String, usize>,
    constants: HashMap<String, Val>,
    enums: HashMap<String, Vec<String>>,
    quit: bool,
    /// Captured `Debug.Print` lines when running `Function Main()`.
    stdout: String,
}

impl Host {
    fn screen(&self) -> &Screen {
        &self.program.screens[self.screen_idx]
    }

    fn key(name: &str) -> String {
        name.to_ascii_lowercase()
    }

    fn get(&self, locals: &HashMap<String, Val>, name: &str) -> Option<Val> {
        let k = Self::key(name);
        locals
            .get(&k)
            .cloned()
            .or_else(|| self.state.get(&k).cloned())
            .or_else(|| self.constants.get(&k).cloned())
    }

    fn set(&mut self, locals: &mut HashMap<String, Val>, name: &str, val: Val) {
        let k = Self::key(name);
        if locals.contains_key(&k) {
            locals.insert(k, val);
        } else if self.state.contains_key(&k) {
            self.state.insert(k, val);
        } else {
            locals.insert(k, val);
        }
    }

    fn get_list_mut<'a>(
        &'a mut self,
        locals: &'a mut HashMap<String, Val>,
        name: &str,
    ) -> Option<&'a mut Vec<Val>> {
        let k = Self::key(name);
        if locals.contains_key(&k) {
            return match locals.get_mut(&k) {
                Some(Val::List(xs)) => Some(xs),
                _ => None,
            };
        }
        match self.state.get_mut(&k) {
            Some(Val::List(xs)) => Some(xs),
            _ => None,
        }
    }

    fn default_val(&self, ty: &DeclType) -> Val {
        match ty {
            DeclType::Plain(Type::Boolean) => Val::Bool(false),
            DeclType::Plain(Type::Single | Type::Double) => Val::Float(0.0),
            DeclType::Plain(Type::Text) => Val::Str(String::new()),
            DeclType::Plain(_) => Val::Int(0),
            DeclType::Vec(_) => Val::List(Vec::new()),
            DeclType::Named(n) => {
                if let Some(vs) = self.enums.get(&Self::key(n)) {
                    if let Some(first) = vs.first() {
                        return Val::Enum {
                            ty: n.clone(),
                            variant: first.clone(),
                        };
                    }
                }
                Val::Null
            }
            _ => Val::Null,
        }
    }

    fn eval(&mut self, locals: &mut HashMap<String, Val>, e: &Expr) -> Result<Val, String> {
        match &e.kind {
            ExprKind::Int(n) => Ok(Val::Int(*n)),
            ExprKind::Float(n) => Ok(Val::Float(*n)),
            ExprKind::Bool(b) => Ok(Val::Bool(*b)),
            ExprKind::Str(s) => Ok(Val::Str(s.clone())),
            ExprKind::Ident(name) => {
                if name.eq_ignore_ascii_case("true") {
                    return Ok(Val::Bool(true));
                }
                if name.eq_ignore_ascii_case("false") {
                    return Ok(Val::Bool(false));
                }
                if name.eq_ignore_ascii_case("nothing") {
                    return Ok(Val::Null);
                }
                self.get(locals, name)
                    .ok_or_else(|| format!("unknown name `{name}`"))
            }
            ExprKind::ConstRef(name) => self
                .get(locals, name)
                .ok_or_else(|| format!("unknown constant `{name}`")),
            ExprKind::Not(inner) => Ok(Val::Bool(!self.eval(locals, inner)?.as_bool())),
            ExprKind::Deref(inner) | ExprKind::Ref(inner) | ExprKind::MutRef(inner) => {
                self.eval(locals, inner)
            }
            ExprKind::Cast(inner, ty) => {
                let v = self.eval(locals, inner)?;
                Ok(match ty {
                    Type::Boolean => Val::Bool(v.as_bool()),
                    Type::Text => Val::Str(v.stringify()),
                    Type::Single | Type::Double => Val::Float(v.as_float()),
                    _ => Val::Int(v.as_int()),
                })
            }
            ExprKind::Try(inner) => self.eval(locals, inner),
            ExprKind::Binary { op, lhs, rhs } => self.eval_bin(locals, *op, lhs, rhs),
            ExprKind::Field(recv, field) => {
                if let ExprKind::Ident(ty) = &recv.kind {
                    if self.enums.contains_key(&Self::key(ty)) {
                        return Ok(Val::Enum {
                            ty: ty.clone(),
                            variant: field.clone(),
                        });
                    }
                }
                Err(format!("field `.{field}` isn't available in the phone Screen host"))
            }
            ExprKind::List(items) => {
                let mut out = Vec::new();
                for it in items {
                    out.push(self.eval(locals, it)?);
                }
                Ok(Val::List(out))
            }
            ExprKind::Index(recv, idx) => {
                let list = self.eval(locals, recv)?;
                let i = self.eval(locals, idx)?.as_int();
                match list {
                    Val::List(xs) => {
                        if i < 0 || i as usize >= xs.len() {
                            Err("list index out of range".into())
                        } else {
                            Ok(xs[i as usize].clone())
                        }
                    }
                    Val::Str(s) => {
                        let ch = s.chars().nth(i as usize).unwrap_or('\0');
                        Ok(Val::Str(ch.to_string()))
                    }
                    _ => Err("index into a non-list".into()),
                }
            }
            ExprKind::Call { name, args } => self.call(locals, name, args),
            ExprKind::MethodCall { recv, method, args } => {
                self.method(locals, recv, method, args)
            }
            ExprKind::Tuple(xs) => {
                let mut out = Vec::new();
                for x in xs {
                    out.push(self.eval(locals, x)?);
                }
                Ok(Val::List(out))
            }
            other => Err(format!(
                "the phone Screen host doesn't evaluate this expression yet ({other:?})"
            )),
        }
    }

    fn eval_bin(
        &mut self,
        locals: &mut HashMap<String, Val>,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<Val, String> {
        if matches!(op, BinOp::And) {
            let l = self.eval(locals, lhs)?;
            if !l.as_bool() {
                return Ok(Val::Bool(false));
            }
            return Ok(Val::Bool(self.eval(locals, rhs)?.as_bool()));
        }
        if matches!(op, BinOp::Or) {
            let l = self.eval(locals, lhs)?;
            if l.as_bool() {
                return Ok(Val::Bool(true));
            }
            return Ok(Val::Bool(self.eval(locals, rhs)?.as_bool()));
        }
        let l = self.eval(locals, lhs)?;
        let r = self.eval(locals, rhs)?;
        match op {
            BinOp::Concat => Ok(Val::Str(format!("{}{}", l.stringify(), r.stringify()))),
            BinOp::Eq => Ok(Val::Bool(eq_val(&l, &r))),
            BinOp::Ne => Ok(Val::Bool(!eq_val(&l, &r))),
            BinOp::Lt => Ok(Val::Bool(cmp_num(&l, &r) < 0)),
            BinOp::Gt => Ok(Val::Bool(cmp_num(&l, &r) > 0)),
            BinOp::Le => Ok(Val::Bool(cmp_num(&l, &r) <= 0)),
            BinOp::Ge => Ok(Val::Bool(cmp_num(&l, &r) >= 0)),
            BinOp::Xor => Ok(Val::Bool(l.as_bool() ^ r.as_bool())),
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                arith(op, &l, &r)
            }
            BinOp::And | BinOp::Or => unreachable!(),
        }
    }

    fn method(
        &mut self,
        locals: &mut HashMap<String, Val>,
        recv: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Result<Val, String> {
        if method.eq_ignore_ascii_case("push") {
            if let ExprKind::Ident(name) = &recv.kind {
                let arg = if args.is_empty() {
                    Val::Null
                } else {
                    self.eval(locals, &args[0])?
                };
                if let Some(xs) = self.get_list_mut(locals, name) {
                    xs.push(arg);
                    return Ok(Val::Null);
                }
            }
            return Err(format!("Push needs a list field, not {recv:?}"));
        }
        let recv_v = self.eval(locals, recv)?;
        if method.eq_ignore_ascii_case("len")
            || method.eq_ignore_ascii_case("length")
            || method.eq_ignore_ascii_case("count")
        {
            return Ok(Val::Int(match recv_v {
                Val::List(xs) => xs.len() as i64,
                Val::Str(s) => s.chars().count() as i64,
                _ => 0,
            }));
        }
        if method.eq_ignore_ascii_case("clone") {
            return Ok(recv_v);
        }
        if method.eq_ignore_ascii_case("tostring") || method.eq_ignore_ascii_case("text") {
            return Ok(Val::Str(recv_v.stringify()));
        }
        Err(format!(".{method}() isn't in the phone Screen host yet"))
    }

    fn call(
        &mut self,
        _locals: &mut HashMap<String, Val>,
        name: &str,
        args: &[Expr],
    ) -> Result<Val, String> {
        if name.eq_ignore_ascii_case("GetOpenFilename")
            || name.eq_ignore_ascii_case("GetSaveAsFilename")
        {
            return Ok(Val::Str(String::new()));
        }
        let argv = args_eval(self, _locals, args)?;
        if let Some(v) = math_builtin(name, &argv) {
            return Ok(v);
        }
        if !self.program.screens.is_empty() {
            if let Some(ev) = self
                .screen()
                .subs
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .cloned()
            {
                return self.run_handler(&ev, argv);
            }
        }
        let func = self
            .program
            .functions
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        let mut locals = HashMap::new();
        for (p, a) in func.params.iter().zip(argv.into_iter()) {
            locals.insert(Self::key(&p.name), a);
        }
        match self.exec_body(&mut locals, &func.body)? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Val::Null),
        }
    }

    fn exec_body(
        &mut self,
        locals: &mut HashMap<String, Val>,
        body: &[Stmt],
    ) -> Result<Flow, String> {
        for stmt in body {
            match self.exec_stmt(locals, stmt)? {
                Flow::Next => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    fn exec_stmt(
        &mut self,
        locals: &mut HashMap<String, Val>,
        stmt: &Stmt,
    ) -> Result<Flow, String> {
        match stmt {
            Stmt::LineMark(_) | Stmt::Comment(_) => Ok(Flow::Next),
            Stmt::Log(_, e) => {
                let _ = self.eval(locals, e)?;
                Ok(Flow::Next)
            }
            Stmt::Print(e) => {
                let v = self.eval(locals, e)?;
                self.stdout.push_str(&v.stringify());
                self.stdout.push('\n');
                Ok(Flow::Next)
            }
            Stmt::Dim { name, ty, init, .. } => {
                let v = if let Some(e) = init {
                    self.eval(locals, e)?
                } else {
                    self.default_val(ty)
                };
                locals.insert(Self::key(name), v);
                Ok(Flow::Next)
            }
            Stmt::Set { name, value, .. } => {
                let v = self.eval(locals, value)?;
                self.set(locals, name, v);
                Ok(Flow::Next)
            }
            Stmt::Assign { target, value, op } => {
                let rhs = self.eval(locals, value)?;
                let name = ident_of(target).ok_or("assignment target isn't a name")?;
                let val = if let Some(op) = op {
                    let lhs = self
                        .get(locals, name)
                        .ok_or_else(|| format!("unknown name `{name}`"))?;
                    arith(*op, &lhs, &rhs)?
                } else {
                    rhs
                };
                self.set(locals, name, val);
                Ok(Flow::Next)
            }
            Stmt::Return(e) => {
                let v = if let Some(e) = e {
                    self.eval(locals, e)?
                } else {
                    Val::Null
                };
                Ok(Flow::Return(v))
            }
            Stmt::Expr(e) => {
                self.eval(locals, e)?;
                Ok(Flow::Next)
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    if self.eval(locals, cond)?.as_bool() {
                        return self.exec_body(locals, body);
                    }
                }
                if let Some(body) = else_body {
                    return self.exec_body(locals, body);
                }
                Ok(Flow::Next)
            }
            Stmt::For {
                var,
                from,
                to,
                step,
                body,
                ty,
            } => {
                if ty.is_float() {
                    // Floating bounds/Step: counted loop, same walk as the
                    // transpiler's `emit_counted_for`.
                    let mut i = self.eval(locals, from)?.as_float();
                    let end = self.eval(locals, to)?.as_float();
                    let step = step
                        .as_ref()
                        .map(|s| self.eval(locals, s).map(|v| v.as_float()))
                        .transpose()?
                        .unwrap_or(1.0);
                    if step == 0.0 {
                        return Err("For step cannot be 0".into());
                    }
                    loop {
                        if (step > 0.0 && i > end) || (step < 0.0 && i < end) {
                            break;
                        }
                        locals.insert(Self::key(var), Val::Float(i));
                        match self.exec_body(locals, body)? {
                            Flow::Break => break,
                            Flow::Continue => {}
                            Flow::Return(v) => return Ok(Flow::Return(v)),
                            Flow::Next => {}
                        }
                        i += step;
                    }
                    return Ok(Flow::Next);
                }
                let mut i = self.eval(locals, from)?.as_int();
                let end = self.eval(locals, to)?.as_int();
                let step = step
                    .as_ref()
                    .map(|s| self.eval(locals, s).map(|v| v.as_int()))
                    .transpose()?
                    .unwrap_or(1);
                if step == 0 {
                    return Err("For step cannot be 0".into());
                }
                loop {
                    if step > 0 && i > end {
                        break;
                    }
                    if step < 0 && i < end {
                        break;
                    }
                    locals.insert(Self::key(var), Val::Int(i));
                    match self.exec_body(locals, body)? {
                        Flow::Break => break,
                        Flow::Continue => {}
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next => {}
                    }
                    i += step;
                }
                Ok(Flow::Next)
            }
            Stmt::ForEach {
                var1,
                var2,
                iter,
                body,
            } => {
                let list = self.eval(locals, iter)?;
                let xs = match list {
                    Val::List(xs) => xs,
                    other => vec![other],
                };
                for (idx, item) in xs.into_iter().enumerate() {
                    locals.insert(Self::key(var1), item);
                    if let Some(k) = var2 {
                        locals.insert(Self::key(k), Val::Int(idx as i64));
                    }
                    match self.exec_body(locals, body)? {
                        Flow::Break => break,
                        Flow::Continue => {}
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next => {}
                    }
                }
                Ok(Flow::Next)
            }
            Stmt::DoLoop { cond, body } => {
                loop {
                    if let Some(DoCond::PreWhile(c)) = cond {
                        if !self.eval(locals, c)?.as_bool() {
                            break;
                        }
                    }
                    if let Some(DoCond::PreUntil(c)) = cond {
                        if self.eval(locals, c)?.as_bool() {
                            break;
                        }
                    }
                    match self.exec_body(locals, body)? {
                        Flow::Break => break,
                        Flow::Continue => {}
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next => {}
                    }
                    match cond {
                        Some(DoCond::PostWhile(c)) => {
                            if !self.eval(locals, c)?.as_bool() {
                                break;
                            }
                        }
                        Some(DoCond::PostUntil(c)) => {
                            if self.eval(locals, c)?.as_bool() {
                                break;
                            }
                        }
                        Some(DoCond::PreWhile(_) | DoCond::PreUntil(_)) | None => {}
                    }
                }
                Ok(Flow::Next)
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
            Stmt::Match {
                scrutinee, arms, ..
            } => {
                let scr = self.eval(locals, scrutinee)?;
                for arm in arms {
                    if match_pat(&arm.pattern, &scr) {
                        if let Some(g) = &arm.guard {
                            if !self.eval(locals, g)?.as_bool() {
                                continue;
                            }
                        }
                        return self.exec_body(locals, &arm.body);
                    }
                }
                Ok(Flow::Next)
            }
            Stmt::Destroy { name, .. } => {
                self.set(locals, name, Val::Null);
                Ok(Flow::Next)
            }
            other => Err(format!(
                "the phone Screen host doesn't run this statement yet ({other:?})"
            )),
        }
    }

    fn run_handler(&mut self, ev: &GuiEvent, args: Vec<Val>) -> Result<Val, String> {
        let mut locals = HashMap::new();
        for (p, a) in ev.params.iter().zip(args.into_iter()) {
            locals.insert(Self::key(&p.name), a);
        }
        match self.exec_body(&mut locals, &ev.body)? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Val::Null),
        }
    }

    fn fire(&mut self, name: &str, args: Vec<Val>) -> Result<(), String> {
        if name.eq_ignore_ascii_case("Quit") {
            self.quit = true;
            return Ok(());
        }
        let ev = self
            .screen()
            .events
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| format!("no Event `{name}`"))?;
        self.run_handler(&ev, args)?;
        Ok(())
    }

    fn init_state(&mut self) -> Result<(), String> {
        let fields = self.screen().state.clone();
        let mut locals = HashMap::new();
        for f in &fields {
            let v = if let Some(init) = &f.init {
                self.eval(&mut locals, init)?
            } else {
                self.default_val(&f.ty)
            };
            let k = Self::key(&f.name);
            self.state.insert(k.clone(), v);
            self.field_types.insert(k, f.ty.clone());
        }
        Ok(())
    }

    fn render(&mut self) -> Result<ScreenFrame, String> {
        let title = self
            .screen()
            .title
            .clone()
            .unwrap_or_else(|| self.screen().name.clone());
        let mut locals = HashMap::new();
        let status = if let Some(e) = &self.screen().status.clone() {
            self.eval(&mut locals, e)?.stringify()
        } else {
            String::new()
        };
        let keys: Vec<KeyJson> = self
            .screen()
            .keys
            .iter()
            .map(|k| KeyJson {
                key: k.key.clone(),
                handler: k.handler.clone(),
                label: k
                    .label
                    .clone()
                    .unwrap_or_else(|| k.handler.to_ascii_lowercase()),
            })
            .collect();
        let timers: Vec<TimerJson> = self
            .screen()
            .timers
            .iter()
            .map(|t| TimerJson {
                ms: t.interval_ms,
                handler: t.handler.clone(),
            })
            .collect();
        let menu = self.screen().menu.as_ref().map(|m| {
            m.menus
                .iter()
                .map(|g| MenuJson {
                    title: g.title.clone(),
                    items: g
                        .items
                        .iter()
                        .map(|it| match it {
                            MenuEntry::Separator => MenuItemJson {
                                sep: true,
                                label: String::new(),
                                handler: String::new(),
                            },
                            MenuEntry::Item { label, handler } => MenuItemJson {
                                sep: false,
                                label: label.clone(),
                                handler: handler.clone(),
                            },
                        })
                        .collect(),
                })
                .collect()
        });
        let view_ast = self.screen().view.clone();
        let view = self.render_view(&mut locals, &view_ast, None)?;
        Ok(ScreenFrame {
            ok: true,
            quit: self.quit,
            error: None,
            title,
            status,
            menu,
            keys,
            timers,
            view,
        })
    }

    fn render_view(
        &mut self,
        locals: &mut HashMap<String, Val>,
        node: &ViewNode,
        size: Option<SizeJson>,
    ) -> Result<Json, String> {
        let wrap = |mut obj: Json, size: Option<SizeJson>| {
            if let Some(s) = size {
                if let Some(map) = obj.as_object_mut() {
                    map.insert("size".into(), serde_json::to_value(s).unwrap_or(Json::Null));
                }
            }
            obj
        };
        match node {
            ViewNode::Constrained { size: sz, child } => {
                self.render_view(locals, child, Some(size_json(*sz)))
            }
            ViewNode::Column {
                children,
                spacing,
                padding,
                hatch: _,
            } => {
                let mut kids = Vec::new();
                for c in children {
                    kids.push(self.render_view(locals, c, None)?);
                }
                Ok(wrap(
                    json!({
                        "kind": "column",
                        "spacing": spacing.unwrap_or(0),
                        "padding": padding.unwrap_or(0),
                        "children": kids,
                    }),
                    size,
                ))
            }
            ViewNode::Row {
                children,
                spacing,
                padding,
                hatch: _,
            } => {
                let mut kids = Vec::new();
                for c in children {
                    kids.push(self.render_view(locals, c, None)?);
                }
                Ok(wrap(
                    json!({
                        "kind": "row",
                        "spacing": spacing.unwrap_or(0),
                        "padding": padding.unwrap_or(0),
                        "children": kids,
                    }),
                    size,
                ))
            }
            ViewNode::Frame {
                title,
                children,
                spacing,
                padding,
                // Paint and hatch are Window/desktop-only decorations; a Screen
                // ignores paint and the phone host can't splice raw ratatui.
                paint: _,
                hatch: _,
            } => {
                let t = if let Some(e) = title {
                    self.eval(locals, e)?.stringify()
                } else {
                    String::new()
                };
                let mut kids = Vec::new();
                for c in children {
                    kids.push(self.render_view(locals, c, None)?);
                }
                Ok(wrap(
                    json!({
                        "kind": "frame",
                        "title": t,
                        "spacing": spacing.unwrap_or(0),
                        "padding": padding.unwrap_or(0),
                        "children": kids,
                    }),
                    size,
                ))
            }
            ViewNode::Space {
                horizontal,
                amount,
            } => Ok(wrap(
                json!({
                    "kind": "space",
                    "horizontal": horizontal,
                    "amount": amount,
                }),
                size,
            )),
            ViewNode::Text { content, .. } => Ok(wrap(
                json!({ "kind": "text", "text": self.eval(locals, content)?.stringify() }),
                size,
            )),
            ViewNode::Button {
                label,
                on_click,
                enabled,
                // Paint/hatch are Window-only decorations.
                paint: _,
                hatch: _,
            } => {
                // `Enabled <expr>` false → the button has no handler (Iced's
                // disabled look), same as the desktop targets.
                let on = match enabled {
                    Some(e) => self.eval(locals, e)?.as_bool(),
                    None => true,
                };
                let handler = if on { on_click.clone() } else { None };
                Ok(wrap(
                    json!({
                        "kind": "button",
                        "label": self.eval(locals, label)?.stringify(),
                        "handler": handler,
                    }),
                    size,
                ))
            }
            ViewNode::Checkbox {
                label,
                value,
                on_toggle,
            } => {
                let checked = self
                    .get(locals, value)
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
                Ok(wrap(
                    json!({
                        "kind": "checkbox",
                        "label": self.eval(locals, label)?.stringify(),
                        "field": value,
                        "value": checked,
                        "handler": on_toggle,
                    }),
                    size,
                ))
            }
            ViewNode::Toggler {
                label,
                value,
                on_toggle,
            } => {
                let checked = self
                    .get(locals, value)
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
                Ok(wrap(
                    json!({
                        "kind": "checkbox",
                        "label": self.eval(locals, label)?.stringify(),
                        "field": value,
                        "value": checked,
                        "handler": on_toggle,
                    }),
                    size,
                ))
            }
            ViewNode::Radio {
                label,
                value,
                option,
                on_select,
            } => {
                let opt = self.eval(locals, option)?;
                let cur = self.get(locals, value).unwrap_or(Val::Null);
                Ok(wrap(
                    json!({
                        "kind": "radio",
                        "label": self.eval(locals, label)?.stringify(),
                        "field": value,
                        "option": opt.stringify(),
                        "selected": eq_val(&cur, &opt),
                        "handler": on_select,
                    }),
                    size,
                ))
            }
            ViewNode::Input { field, on_submit } => {
                let v = self
                    .get(locals, field)
                    .map(|v| v.stringify())
                    .unwrap_or_default();
                Ok(wrap(
                    json!({
                        "kind": "input",
                        "field": field,
                        "value": v,
                        "handler": on_submit,
                    }),
                    size,
                ))
            }
            ViewNode::TextInput {
                placeholder,
                value,
                on_input,
                on_submit,
                secure,
            } => {
                let v = self
                    .get(locals, value)
                    .map(|x| x.stringify())
                    .unwrap_or_default();
                Ok(wrap(
                    json!({
                        "kind": "input",
                        "field": value,
                        "value": v,
                        "placeholder": self.eval(locals, placeholder)?.stringify(),
                        // The WebView syncs the field on every keystroke and
                        // fires `handler` on Enter — that's `On Submit` now
                        // (`On Input` before it existed).
                        "handler": on_submit.clone().or_else(|| on_input.clone()),
                        "secure": secure,
                    }),
                    size,
                ))
            }
            ViewNode::Memo { field } | ViewNode::TextArea { value: field } => {
                let v = self
                    .get(locals, field)
                    .map(|x| x.stringify())
                    .unwrap_or_default();
                Ok(wrap(
                    json!({
                        "kind": "memo",
                        "field": field,
                        "value": v,
                    }),
                    size,
                ))
            }
            ViewNode::List {
                field,
                on_select,
                hatch: _,
            } => {
                let items = match self.get(locals, field).unwrap_or(Val::List(Vec::new())) {
                    Val::List(xs) => xs.into_iter().map(|v| v.stringify()).collect::<Vec<_>>(),
                    other => vec![other.stringify()],
                };
                let selected = *self.list_sel.get(&Self::key(field)).unwrap_or(&0);
                Ok(wrap(
                    json!({
                        "kind": "list",
                        "field": field,
                        "items": items,
                        "selected": selected,
                        "handler": on_select,
                    }),
                    size,
                ))
            }
            ViewNode::Tabs {
                field,
                tabs,
                on_change,
                hatch: _,
            } => {
                let idx = self.get(locals, field).map(|v| v.as_int()).unwrap_or(0) as usize;
                let mut out = Vec::new();
                for (i, t) in tabs.iter().enumerate() {
                    let title = self.eval(locals, &t.title)?.stringify();
                    let mut body = Vec::new();
                    if i == idx {
                        for c in &t.children {
                            body.push(self.render_view(locals, c, None)?);
                        }
                    }
                    out.push(json!({ "title": title, "body": body }));
                }
                Ok(wrap(
                    json!({
                        "kind": "tabs",
                        "field": field,
                        "index": idx,
                        "handler": on_change,
                        "tabs": out,
                    }),
                    size,
                ))
            }
            ViewNode::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    if self.eval(locals, cond)?.as_bool() {
                        return self.render_nodes(locals, body, size);
                    }
                }
                if let Some(body) = else_body {
                    return self.render_nodes(locals, body, size);
                }
                Ok(wrap(json!({ "kind": "empty" }), size))
            }
            ViewNode::Match { scrutinee, arms } => {
                let scr = self.eval(locals, scrutinee)?;
                for arm in arms {
                    if match_pat(&arm.pattern, &scr) {
                        if let Some(g) = &arm.guard {
                            if !self.eval(locals, g)?.as_bool() {
                                continue;
                            }
                        }
                        return self.render_nodes(locals, &arm.body, size);
                    }
                }
                Ok(wrap(json!({ "kind": "empty" }), size))
            }
            ViewNode::Gauge { min, max, value } => {
                let lo = self.eval(locals, min)?.as_float();
                let hi = self.eval(locals, max)?.as_float();
                let v = self.get(locals, value).map(|x| x.as_float()).unwrap_or(0.0);
                let span = (hi - lo).abs().max(1e-9);
                let pct = ((v - lo) / span).clamp(0.0, 1.0);
                Ok(wrap(
                    json!({
                        "kind": "gauge",
                        "min": lo,
                        "max": hi,
                        "value": v,
                        "pct": pct,
                    }),
                    size,
                ))
            }
            ViewNode::ProgressBar { min, max, value } => {
                let lo = self.eval(locals, min)?.as_float();
                let hi = self.eval(locals, max)?.as_float();
                let v = self.get(locals, value).map(|x| x.as_float()).unwrap_or(0.0);
                let span = (hi - lo).abs().max(1e-9);
                let pct = ((v - lo) / span).clamp(0.0, 1.0);
                Ok(wrap(
                    json!({
                        "kind": "gauge",
                        "min": lo,
                        "max": hi,
                        "value": v,
                        "pct": pct,
                    }),
                    size,
                ))
            }
            ViewNode::Sparkline { field } => {
                let values: Vec<f64> = match self.get(locals, field).unwrap_or(Val::List(Vec::new())) {
                    Val::List(xs) => xs.iter().map(|v| v.as_float()).collect(),
                    other => vec![other.as_float()],
                };
                Ok(wrap(json!({ "kind": "sparkline", "values": values }), size))
            }
            ViewNode::BarChart { .. } => Ok(wrap(
                json!({
                    "kind": "unsupported",
                    "widget": "BarChart",
                    "hint": "BarChart isn't clickable on the phone yet — Gauge / Sparkline are.",
                }),
                size,
            )),
            ViewNode::Chart { .. } => Ok(wrap(
                json!({
                    "kind": "unsupported",
                    "widget": "Chart",
                    "hint": "Chart isn't clickable on the phone yet — Gauge / Sparkline are.",
                }),
                size,
            )),
            ViewNode::Table { field, .. } => Ok(wrap(
                json!({
                    "kind": "unsupported",
                    "widget": "Table",
                    "hint": format!("Table `{field}` isn't on the phone Screen yet — List is."),
                }),
                size,
            )),
            ViewNode::Slider {
                min,
                max,
                value,
                on_change,
                step,
                // `Vertical` is an Iced layout mode; the phone host draws a
                // horizontal range slider either way.
                vertical: _,
            } => {
                let lo = self.eval(locals, min)?.as_float();
                let hi = self.eval(locals, max)?.as_float();
                let v = self.get(locals, value).map(|x| x.as_float()).unwrap_or(lo);
                let step = match step {
                    Some(e) => serde_json::to_value(self.eval(locals, e)?.as_float())
                        .unwrap_or(Json::Null),
                    None => Json::Null,
                };
                Ok(wrap(
                    json!({
                        "kind": "slider",
                        "field": value,
                        "min": lo,
                        "max": hi,
                        "value": v,
                        "handler": on_change,
                        "step": step,
                    }),
                    size,
                ))
            }
            other => Ok(wrap(
                json!({
                    "kind": "unsupported",
                    "widget": format!("{other:?}").split('{').next().unwrap_or("widget").trim(),
                    "hint": "this widget isn't on the phone Screen yet",
                }),
                size,
            )),
        }
    }

    fn render_nodes(
        &mut self,
        locals: &mut HashMap<String, Val>,
        nodes: &[ViewNode],
        size: Option<SizeJson>,
    ) -> Result<Json, String> {
        if nodes.len() == 1 {
            return self.render_view(locals, &nodes[0], size);
        }
        let mut kids = Vec::new();
        for n in nodes {
            kids.push(self.render_view(locals, n, None)?);
        }
        Ok(json!({ "kind": "column", "spacing": 0, "padding": 0, "children": kids, "size": size }))
    }

    fn option_value(&self, field: &str, option: &str) -> Val {
        let k = Self::key(field);
        if let Some(DeclType::Named(ty)) = self.field_types.get(&k) {
            return Val::Enum {
                ty: ty.clone(),
                variant: option.to_string(),
            };
        }
        if let Ok(n) = option.parse::<i64>() {
            return Val::Int(n);
        }
        if option.eq_ignore_ascii_case("true") || option.eq_ignore_ascii_case("false") {
            return Val::Bool(option.eq_ignore_ascii_case("true"));
        }
        Val::Str(option.to_string())
    }

    fn dispatch(&mut self, ev: &Json) -> Result<(), String> {
        let op = ev.get("op").and_then(|v| v.as_str()).unwrap_or("");
        match op {
            "quit" => {
                self.quit = true;
                Ok(())
            }
            "event" | "click" | "key" | "menu" => {
                let name = ev
                    .get("name")
                    .or_else(|| ev.get("handler"))
                    .and_then(|v| v.as_str())
                    .ok_or("event needs a name")?;
                let args = json_args(ev.get("args"));
                self.fire(name, args)
            }
            "toggle" => {
                let field = ev.get("field").and_then(|v| v.as_str()).ok_or("toggle needs field")?;
                let cur = self.get(&HashMap::new(), field).unwrap_or(Val::Bool(false));
                let next = Val::Bool(!cur.as_bool());
                self.state.insert(Self::key(field), next.clone());
                if let Some(h) = ev.get("handler").and_then(|v| v.as_str()) {
                    if !h.is_empty() {
                        self.fire(h, vec![next])?;
                    }
                }
                Ok(())
            }
            "radio" => {
                let field = ev.get("field").and_then(|v| v.as_str()).ok_or("radio needs field")?;
                let option = ev
                    .get("option")
                    .and_then(|v| v.as_str())
                    .ok_or("radio needs option")?;
                let val = self.option_value(field, option);
                self.state.insert(Self::key(field), val.clone());
                if let Some(h) = ev.get("handler").and_then(|v| v.as_str()) {
                    if !h.is_empty() {
                        self.fire(h, vec![val])?;
                    }
                }
                Ok(())
            }
            "list" => {
                let field = ev.get("field").and_then(|v| v.as_str()).ok_or("list needs field")?;
                let index = ev.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.list_sel.insert(Self::key(field), index);
                let item = match self.get(&HashMap::new(), field) {
                    Some(Val::List(xs)) => xs.get(index).cloned().unwrap_or(Val::Null),
                    other => other.unwrap_or(Val::Null),
                };
                if let Some(h) = ev.get("handler").and_then(|v| v.as_str()) {
                    if !h.is_empty() {
                        self.fire(h, vec![item])?;
                    }
                }
                Ok(())
            }
            "input" => {
                let field = ev.get("field").and_then(|v| v.as_str()).ok_or("input needs field")?;
                let value = ev
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                self.state.insert(Self::key(field), Val::Str(value));
                Ok(())
            }
            "submit" => {
                let field = ev.get("field").and_then(|v| v.as_str()).ok_or("submit needs field")?;
                let value = self
                    .get(&HashMap::new(), field)
                    .unwrap_or(Val::Str(String::new()));
                if let Some(h) = ev.get("handler").and_then(|v| v.as_str()) {
                    if !h.is_empty() {
                        self.fire(h, vec![value])?;
                    }
                }
                Ok(())
            }
            "tab" => {
                let field = ev.get("field").and_then(|v| v.as_str()).ok_or("tab needs field")?;
                let index = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
                self.state.insert(Self::key(field), Val::Int(index));
                if let Some(h) = ev.get("handler").and_then(|v| v.as_str()) {
                    if !h.is_empty() {
                        self.fire(h, vec![Val::Int(index)])?;
                    }
                }
                Ok(())
            }
            "slider" => {
                let field = ev.get("field").and_then(|v| v.as_str()).ok_or("slider needs field")?;
                let value = ev.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let stored = if value.fract() == 0.0 {
                    Val::Int(value as i64)
                } else {
                    Val::Float(value)
                };
                self.state.insert(Self::key(field), stored.clone());
                if let Some(h) = ev.get("handler").and_then(|v| v.as_str()) {
                    if !h.is_empty() {
                        self.fire(h, vec![stored])?;
                    }
                }
                Ok(())
            }
            _ => Err(format!("unknown screen op `{op}`")),
        }
    }
}

fn args_eval(
    host: &mut Host,
    locals: &mut HashMap<String, Val>,
    args: &[Expr],
) -> Result<Vec<Val>, String> {
    let mut out = Vec::new();
    for a in args {
        out.push(host.eval(locals, a)?);
    }
    Ok(out)
}

fn math_builtin(name: &str, args: &[Val]) -> Option<Val> {
    let x = args.first()?.as_float();
    Some(Val::Float(match name.to_ascii_lowercase().as_str() {
        "sqr" => x.sqrt(),
        "abs" => x.abs(),
        "int" => x.floor(),
        "round" => x.round(),
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "atn" => x.atan(),
        "exp" => x.exp(),
        "log" => x.ln(),
        _ => return None,
    }))
}

fn ident_of(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Ident(n) => Some(n.as_str()),
        ExprKind::Field(recv, f) if f.eq_ignore_ascii_case("field") => ident_of(recv),
        _ => None,
    }
}

fn eq_val(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Enum { variant: x, .. }, Val::Enum { variant: y, .. }) => {
            x.eq_ignore_ascii_case(y)
        }
        (Val::Enum { variant, .. }, Val::Str(s)) | (Val::Str(s), Val::Enum { variant, .. }) => {
            variant.eq_ignore_ascii_case(s)
        }
        (Val::Str(x), Val::Str(y)) => x == y,
        (Val::Bool(x), Val::Bool(y)) => x == y,
        (Val::Int(x), Val::Int(y)) => x == y,
        (Val::Float(x), Val::Float(y)) => (x - y).abs() < 1e-9,
        (Val::Null, Val::Null) => true,
        (Val::List(x), Val::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| eq_val(a, b)),
        (l, r) if l.is_floatish() || r.is_floatish() => (l.as_float() - r.as_float()).abs() < 1e-9,
        (Val::Int(_) | Val::Float(_) | Val::Bool(_), Val::Int(_) | Val::Float(_) | Val::Bool(_)) => {
            a.as_int() == b.as_int()
        }
        _ => a.stringify() == b.stringify(),
    }
}

fn cmp_num(a: &Val, b: &Val) -> i32 {
    let d = a.as_float() - b.as_float();
    if d < 0.0 {
        -1
    } else if d > 0.0 {
        1
    } else {
        0
    }
}

fn arith(op: BinOp, l: &Val, r: &Val) -> Result<Val, String> {
    let floaty = l.is_floatish() || r.is_floatish() || matches!(op, BinOp::Pow);
    if floaty {
        let a = l.as_float();
        let b = r.as_float();
        let n = match op {
            BinOp::Add => a + b,
            BinOp::Sub => a - b,
            BinOp::Mul => a * b,
            BinOp::Div => {
                if b == 0.0 {
                    return Err("division by zero".into());
                }
                a / b
            }
            BinOp::Mod => a % b,
            BinOp::Pow => a.powf(b),
            _ => 0.0,
        };
        Ok(Val::Float(n))
    } else {
        let a = l.as_int();
        let b = r.as_int();
        let n = match op {
            BinOp::Add => a.saturating_add(b),
            BinOp::Sub => a.saturating_sub(b),
            BinOp::Mul => a.saturating_mul(b),
            BinOp::Div => {
                if b == 0 {
                    return Err("division by zero".into());
                }
                a / b
            }
            BinOp::Mod => {
                if b == 0 {
                    return Err("division by zero".into());
                }
                a % b
            }
            BinOp::Pow => a.saturating_pow(b.max(0) as u32),
            _ => 0,
        };
        Ok(Val::Int(n))
    }
}

fn match_pat(pattern: &str, v: &Val) -> bool {
    let p = pattern.trim();
    if p == "_" {
        return true;
    }
    let got = v.stringify();
    if p.eq_ignore_ascii_case(&got) {
        return true;
    }
    if let Some((_, rest)) = p.rsplit_once("::") {
        return rest.trim().eq_ignore_ascii_case(&got);
    }
    if let Some((_, rest)) = p.rsplit_once('.') {
        return rest.trim().eq_ignore_ascii_case(&got);
    }
    false
}

fn json_args(v: Option<&Json>) -> Vec<Val> {
    match v {
        Some(Json::Array(xs)) => xs.iter().map(json_to_val).collect(),
        Some(other) => vec![json_to_val(other)],
        None => Vec::new(),
    }
}

fn json_to_val(v: &Json) -> Val {
    match v {
        Json::Null => Val::Null,
        Json::Bool(b) => Val::Bool(*b),
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                Val::Int(i)
            } else {
                Val::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        Json::String(s) => Val::Str(s.clone()),
        Json::Array(xs) => Val::List(xs.iter().map(json_to_val).collect()),
        Json::Object(_) => Val::Str(v.to_string()),
    }
}

fn size_json(s: SizeConstraint) -> SizeJson {
    match s {
        SizeConstraint::Length(n) => SizeJson {
            kind: "length",
            n,
        },
        SizeConstraint::Percent(n) => SizeJson {
            kind: "percent",
            n,
        },
        SizeConstraint::Fill(n) => SizeJson { kind: "fill", n },
        SizeConstraint::Min(n) => SizeJson { kind: "min", n },
    }
}

fn parse_program(source: &str) -> Result<Program, String> {
    let mut diags = Diagnostics::new();
    let program = parser::parse(lexer::lex(source), &mut diags);
    if diags.has_errors() {
        let msg = diags
            .items()
            .iter()
            .filter(|d| matches!(d.level, vbr::diagnostics::Level::Error))
            .map(|d| d.render())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(if msg.is_empty() {
            "couldn't parse this Screen".into()
        } else {
            msg
        });
    }
    Ok(program)
}

fn new_host(program: Program, screen_idx: usize) -> Result<Host, String> {
    let mut enums = HashMap::new();
    for e in &program.enums {
        enums.insert(
            Host::key(&e.name),
            e.variants.iter().map(|v| v.name.clone()).collect(),
        );
    }
    let mut host = Host {
        program,
        screen_idx,
        state: HashMap::new(),
        field_types: HashMap::new(),
        list_sel: HashMap::new(),
        constants: HashMap::new(),
        enums,
        quit: false,
        stdout: String::new(),
    };
    let consts = host.program.constants.clone();
    let mut constants = HashMap::new();
    {
        let mut locals = HashMap::new();
        for c in &consts {
            constants.insert(Host::key(&c.name), host.eval(&mut locals, &c.value)?);
        }
    }
    host.constants = constants;
    Ok(host)
}

/// Run `Function Main()` and collect `Debug.Print` output.
///
/// Android will not give an ordinary app RWX memory for TinyCC's in-process
/// JIT (`tcc_relocate` hangs). This is the same AST host Screen already uses.
pub fn run_main(source: &str) -> Result<String, String> {
    let program = parse_program(source)?;
    let body = program
        .functions
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case("Main"))
        .map(|f| f.body.clone())
        .ok_or_else(|| "this program has no Function Main() to run".to_string())?;
    let mut host = new_host(program, 0)?;
    let mut locals = HashMap::new();
    match host.exec_body(&mut locals, &body) {
        Ok(Flow::Return(_) | Flow::Next | Flow::Break | Flow::Continue) => Ok(host.stdout),
        Err(e) => Err(e),
    }
}

fn launched_name(program: &Program) -> Option<String> {
    let main = program
        .functions
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case("Main"))?;
    for stmt in &main.body {
        if let Stmt::Expr(e) = stmt {
            let (recv, method) = match &e.kind {
                ExprKind::Field(recv, m) => (recv.as_ref(), m.as_str()),
                ExprKind::MethodCall { recv, method, .. } => (recv.as_ref(), method.as_str()),
                _ => continue,
            };
            if !method.eq_ignore_ascii_case("run") {
                continue;
            }
            if let ExprKind::Ident(name) = &recv.kind {
                return Some(name.clone());
            }
        }
    }
    None
}

/// `"screen"` / `"window"` / `"page"` when the source launches that surface.
pub fn detect_surface(source: &str) -> Option<&'static str> {
    let program = parse_program(source).ok()?;
    if let Some(name) = launched_name(&program) {
        if program.screens.iter().any(|s| s.name.eq_ignore_ascii_case(&name)) {
            return Some("screen");
        }
        if program.windows.iter().any(|w| w.name.eq_ignore_ascii_case(&name)) {
            return Some("window");
        }
        if program.pages.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
            return Some("page");
        }
    }
    if !program.screens.is_empty() {
        return Some("screen");
    }
    if !program.windows.is_empty() {
        return Some("window");
    }
    if !program.pages.is_empty() {
        return Some("page");
    }
    None
}

fn lock_session() -> std::sync::MutexGuard<'static, Option<Host>> {
    SESSION.lock().unwrap_or_else(|e| e.into_inner())
}

fn frame_err(msg: impl Into<String>) -> ScreenFrame {
    ScreenFrame {
        ok: false,
        quit: false,
        error: Some(msg.into()),
        title: String::new(),
        status: String::new(),
        menu: None,
        keys: Vec::new(),
        timers: Vec::new(),
        view: Json::Null,
    }
}

fn to_json(frame: ScreenFrame) -> String {
    serde_json::to_string(&frame).unwrap_or_else(|e| {
        format!("{{\"ok\":false,\"quit\":false,\"error\":\"{e}\",\"title\":\"\",\"status\":\"\",\"keys\":[],\"timers\":[],\"view\":null}}")
    })
}

pub fn screen_start(source: &str) -> String {
    let program = match parse_program(source) {
        Ok(p) => p,
        Err(e) => return to_json(frame_err(e)),
    };
    if program.screens.is_empty() {
        return to_json(frame_err("this program has no Screen"));
    }
    let idx = launched_name(&program)
        .and_then(|n| {
            program
                .screens
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(&n))
        })
        .unwrap_or(0);
    let mut host = match new_host(program, idx) {
        Ok(h) => h,
        Err(e) => return to_json(frame_err(e)),
    };
    if let Err(e) = host.init_state() {
        return to_json(frame_err(e));
    }
    let frame = match host.render() {
        Ok(f) => f,
        Err(e) => return to_json(frame_err(e)),
    };
    *lock_session() = Some(host);
    to_json(frame)
}

pub fn screen_dispatch(event_json: &str) -> String {
    let ev: Json = match serde_json::from_str(event_json) {
        Ok(v) => v,
        Err(e) => return to_json(frame_err(format!("bad screen event: {e}"))),
    };
    let mut guard = lock_session();
    let Some(host) = guard.as_mut() else {
        return to_json(frame_err("no Screen is running — F9 a Screen program first"));
    };
    if let Err(e) = host.dispatch(&ev) {
        let mut frame = match host.render() {
            Ok(f) => f,
            Err(re) => frame_err(re),
        };
        frame.ok = false;
        frame.error = Some(e);
        return to_json(frame);
    }
    match host.render() {
        Ok(f) => {
            let json = to_json(f);
            if host.quit {
                *guard = None;
            }
            json
        }
        Err(e) => to_json(frame_err(e)),
    }
}

pub fn screen_stop() {
    *lock_session() = None;
}

#[derive(Serialize)]
pub struct ScreenFrame {
    pub ok: bool,
    pub quit: bool,
    pub error: Option<String>,
    pub title: String,
    pub status: String,
    pub menu: Option<Vec<MenuJson>>,
    pub keys: Vec<KeyJson>,
    pub timers: Vec<TimerJson>,
    pub view: Json,
}

#[derive(Serialize)]
pub struct MenuJson {
    pub title: String,
    pub items: Vec<MenuItemJson>,
}

#[derive(Serialize)]
pub struct MenuItemJson {
    pub sep: bool,
    pub label: String,
    pub handler: String,
}

#[derive(Serialize)]
pub struct KeyJson {
    pub key: String,
    pub handler: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct TimerJson {
    pub ms: u64,
    pub handler: String,
}

#[derive(Serialize, Clone)]
struct SizeJson {
    kind: &'static str,
    n: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn start(src: &str) -> ScreenFrame {
        screen_stop();
        serde_json::from_str(&screen_start(src)).expect("frame json")
    }

    fn send(op: &str) -> ScreenFrame {
        serde_json::from_str(&screen_dispatch(op)).expect("frame json")
    }

    fn texts(v: &Json, out: &mut Vec<String>) {
        match v {
            Json::Object(m) => {
                if let Some(Json::String(t)) = m.get("text") {
                    out.push(t.clone());
                }
                if let Some(Json::String(t)) = m.get("label") {
                    out.push(t.clone());
                }
                if let Some(Json::String(t)) = m.get("status") {
                    out.push(t.clone());
                }
                for val in m.values() {
                    texts(val, out);
                }
            }
            Json::Array(xs) => {
                for x in xs {
                    texts(x, out);
                }
            }
            _ => {}
        }
    }

    fn blob(f: &ScreenFrame) -> String {
        let mut t = vec![f.title.clone(), f.status.clone()];
        texts(&f.view, &mut t);
        t.join(" | ")
    }

    impl<'de> serde::Deserialize<'de> for ScreenFrame {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let v = Json::deserialize(d)?;
            Ok(ScreenFrame {
                ok: v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
                quit: v.get("quit").and_then(|x| x.as_bool()).unwrap_or(false),
                error: v
                    .get("error")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                title: v
                    .get("title")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .into(),
                status: v
                    .get("status")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .into(),
                menu: None,
                keys: Vec::new(),
                timers: Vec::new(),
                view: v.get("view").cloned().unwrap_or(Json::Null),
            })
        }
    }

    #[test]
    fn counter_clicks_like_plus_key() {
        let _g = lock();
        let src = include_str!("../../../examples/tui_counter.vbr");
        assert_eq!(detect_surface(src), Some("screen"));
        let f = start(src);
        assert!(f.ok, "{:?}", f.error);
        assert!(blob(&f).contains("Count: 0"), "{}", blob(&f));
        let f = send(r#"{"op":"event","name":"Increment"}"#);
        assert!(blob(&f).contains("Count: 1"), "{}", blob(&f));
        let f = send(r#"{"op":"event","name":"Decrement"}"#);
        assert!(blob(&f).contains("Count: 0"), "{}", blob(&f));
        let f = send(r#"{"op":"quit"}"#);
        assert!(f.quit);
    }

    #[test]
    fn controls_button_checkbox_radio() {
        let _g = lock();
        let src = include_str!("../../../examples/tui_controls.vbr");
        let f = start(src);
        assert!(f.ok, "{:?}", f.error);
        let f = send(r#"{"op":"click","handler":"Bumped"}"#);
        assert!(blob(&f).contains("clicks: 1"), "{}", blob(&f));
        let f = send(r#"{"op":"toggle","field":"remember","handler":"Toggled"}"#);
        assert!(f.status.contains("checkbox on"), "{}", f.status);
        let f = send(r#"{"op":"radio","field":"choice","option":"Large","handler":"Picked"}"#);
        assert!(f.status.contains("radio changed"), "{}", f.status);
        screen_stop();
    }

    #[test]
    fn list_select_sets_choice() {
        let _g = lock();
        let src = include_str!("../../../examples/tui_list.vbr");
        let f = start(src);
        assert!(f.ok, "{:?}", f.error);
        let dumped = f.view.to_string();
        assert!(dumped.contains("Apple"), "{dumped}");
        let f = send(r#"{"op":"list","field":"fruits","index":2,"handler":"Choose"}"#);
        assert!(blob(&f).contains("Cherry"), "{}", blob(&f));
        screen_stop();
    }

    #[test]
    fn input_submit_pushes_note() {
        let _g = lock();
        let src = include_str!("../../../examples/tui_input.vbr");
        let f = start(src);
        assert!(f.ok, "{:?}", f.error);
        send(r#"{"op":"input","field":"entry","value":"write tests"}"#);
        let f = send(r#"{"op":"submit","field":"entry","handler":"Add"}"#);
        assert!(blob(&f).contains("write tests"), "{}", blob(&f));
        screen_stop();
    }

    #[test]
    fn tabs_and_view_if() {
        let _g = lock();
        let src = include_str!("../../../examples/tui_tabs.vbr");
        let f = start(src);
        assert!(f.ok, "{:?}", f.error);
        assert!(blob(&f).contains("idle"), "{}", blob(&f));
        let f = send(r#"{"op":"tab","field":"tab","index":2}"#);
        assert!(blob(&f).contains("Busy"), "{}", blob(&f));
        let _f = send(r#"{"op":"toggle","field":"busy","handler":"Toggled"}"#);
        let f = send(r#"{"op":"tab","field":"tab","index":0}"#);
        assert!(blob(&f).contains("working"), "{}", blob(&f));
        screen_stop();
    }

    #[test]
    fn pulse_timer_event_moves_gauge() {
        let _g = lock();
        let src = include_str!("../../../examples/tui_pulse.vbr");
        let f = start(src);
        assert!(f.ok, "{:?}", f.error);
        let f = send(r#"{"op":"event","name":"Beat"}"#);
        let dumped = f.view.to_string();
        assert!(dumped.contains("\"value\":5") || dumped.contains("\"value\":5.0"), "{dumped}");
        screen_stop();
    }

    #[test]
    fn run_main_prints_hello() {
        let out = run_main(
            "Function Main()\n    Debug.Print \"hello, android\"\nEnd Function\n",
        )
        .expect("Main");
        assert!(
            out.contains("hello, android"),
            "stdout was: {out:?}"
        );
    }

    #[test]
    fn run_main_maths_example() {
        let out = run_main(include_str!("../../../examples/maths.vbr")).expect("Main");
        assert!(out.contains("sqrt(9)"), "stdout was: {out:?}");
        assert!(out.contains("17 Mod 5"), "stdout was: {out:?}");
    }
}
