//! Recursive-descent parser: tokens in, AST out.
//!
//! On an unexpected token the parser records an `✘` diagnostic and unwinds via
//! `Option`/`?`, so a malformed file produces a clear message instead of a panic.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::lexer::{Tok, Token};
use crate::span::Span;

pub fn parse(tokens: Vec<Token>, diags: &mut Diagnostics) -> Program {
    let mut p = Parser {
        toks: tokens,
        pos: 0,
        diags,
        dim_overflow: Vec::new(),
    };
    p.parse_program()
}

struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    diags: &'a mut Diagnostics,
    /// The extra declarations from a multi-variable `Dim` line
    /// (`Dim a As Long, b As Integer` → one `Stmt` returned, the rest queued
    /// here). Every place that collects statements drains this straight after
    /// the `Dim`, so the second and later variables land right beside the first.
    dim_overflow: Vec<Stmt>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn line(&self) -> usize {
        self.toks[self.pos].line
    }

    /// The current token's source byte range — where an "unexpected token"
    /// style error should point.
    fn span(&self) -> Span {
        self.toks[self.pos].span
    }

    /// The span of the last token consumed — the natural *end* of whatever
    /// was just parsed (a closing paren, a member name).
    fn prev_span(&self) -> Span {
        self.toks[self.pos.saturating_sub(1)].span
    }

    fn advance(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, want: &Tok) -> bool {
        if self.peek() == want {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, want: &Tok, ctx: &str) -> Option<()> {
        if self.peek() == want {
            self.advance();
            Some(())
        } else {
            self.diags.error_at(
                self.span(),
                self.line(),
                format!("Expected {:?} {}, found {:?}.", want, ctx, self.peek()),
            );
            None
        }
    }

    fn expect_ident(&mut self, ctx: &str) -> Option<String> {
        if let Tok::Ident(name) = self.peek().clone() {
            self.advance();
            Some(name)
        } else {
            let found = self.peek().clone();
            let msg = match crate::lexer::keyword_word(&found) {
                Some(kw) => format!(
                    "`{kw}` is a Bust keyword, so it can't be used as a name {ctx}. \
                     Pick another (for example a more descriptive word, or a `_` suffix \
                     like `{}_`).",
                    kw.to_ascii_lowercase()
                ),
                None => format!("Expected a name {}, found {:?}.", ctx, found),
            };
            self.diags.error_at(self.span(), self.line(), msg);
            None
        }
    }

    /// Skip blank lines (and stray comments at structural boundaries).
    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.advance();
        }
    }

    fn parse_program(&mut self) -> Program {
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut enums = Vec::new();
        let mut constants = Vec::new();
        let mut uses = Vec::new();
        let mut windows = Vec::new();
        let mut canvases = Vec::new();
        let mut sketches = Vec::new();
        let mut screens = Vec::new();
        let mut godot_nodes = Vec::new();
        let mut pages = Vec::new();
        let mut css = Vec::new();
        let mut tests = Vec::new();
        let mut top_comments = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Tok::Eof => break,
                // A comment outside any function: keep it as a top-level `//` line.
                Tok::Comment(_) => {
                    if let Tok::Comment(text) = self.advance() {
                        top_comments.push(text);
                    }
                }
                Tok::Use(_) => {
                    if let Some(u) = self.parse_use() {
                        uses.push(u);
                    }
                }
                Tok::Function => match self.parse_function(false, false, false) {
                    Some(f) => functions.push(f),
                    // Error already recorded — resync at the next item so the
                    // rest of the file still gets diagnostics.
                    None => self.recover_to_item(),
                },
                Tok::Ident(w) if w.eq_ignore_ascii_case("Gpu") => {
                    self.advance();
                    match self.peek() {
                        Tok::Function => match self.parse_function(false, false, true) {
                            Some(f) => functions.push(f),
                            None => self.recover_to_item(),
                        },
                        Tok::Sub => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                "`Gpu Sub` isn't a thing — write `Gpu Function` (it can still return nothing).",
                            );
                            self.recover_to_item();
                        }
                        _ => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                "Expected `Gpu Function` or, inside a Sketch, `Gpu Draw`.",
                            );
                            self.recover_to_item();
                        }
                    }
                },
                Tok::Sub => match self.parse_function(false, true, false) {
                    Some(f) => functions.push(f),
                    None => self.recover_to_item(),
                },
                Tok::Type => match self.parse_struct(false) {
                    Some(s) => structs.push(s),
                    None => self.recover_to_item(),
                },
                Tok::Enum => match self.parse_enum(false) {
                    Some(e) => enums.push(e),
                    None => self.recover_to_item(),
                },
                Tok::Const => match self.parse_const(false) {
                    Some(c) => constants.push(c),
                    None => self.recover_to_item(),
                },
                Tok::Public | Tok::Private => {
                    let public = matches!(self.peek(), Tok::Public);
                    self.advance();
                    match self.peek() {
                        Tok::Ident(w) if w.eq_ignore_ascii_case("Gpu") => {
                            self.advance();
                            match self.peek() {
                                Tok::Function => match self.parse_function(public, false, true) {
                                    Some(f) => functions.push(f),
                                    None => self.recover_to_item(),
                                },
                                _ => {
                                    self.diags.error_at(
                                        self.span(),
                                        self.line(),
                                        "Expected `Gpu Function` after `Public Gpu`.",
                                    );
                                    self.recover_to_item();
                                }
                            }
                        }
                        Tok::Function => match self.parse_function(public, false, false) {
                            Some(f) => functions.push(f),
                            None => self.recover_to_item(),
                        },
                        Tok::Sub => match self.parse_function(public, true, false) {
                            Some(f) => functions.push(f),
                            None => self.recover_to_item(),
                        },
                        Tok::Type => match self.parse_struct(public) {
                            Some(s) => structs.push(s),
                            None => self.recover_to_item(),
                        },
                        Tok::Enum => match self.parse_enum(public) {
                            Some(e) => enums.push(e),
                            None => self.recover_to_item(),
                        },
                        Tok::Const => match self.parse_const(public) {
                            Some(c) => constants.push(c),
                            None => self.recover_to_item(),
                        },
                        _ => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                "Module-level variables (global state) aren't supported. Rust \
                                 avoids global mutable state because it makes data races easy to \
                                 write by accident. Instead, pass the value into the functions \
                                 that need it — as a `ByRef` parameter when they must change it — \
                                 or wrap related state in a struct (`Type`) and give it methods. \
                                 (Module-level `Const` is fine — it's immutable and shared safely.)",
                            );
                            self.recover_to_item();
                        }
                    }
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Window") => {
                    if let Some(win) = self.parse_window("Window") {
                        windows.push(win);
                    } else {
                        self.recover_to_item();
                    }
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Page") => {
                    if let Some(pg) = self.parse_window("Page") {
                        pages.push(pg);
                    } else {
                        self.recover_to_item();
                    }
                }
                Tok::InlineCss(_) => {
                    if let Tok::InlineCss(raw) = self.advance() {
                        css.push(raw);
                    }
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Canvas") => {
                    if let Some(cv) = self.parse_canvas() {
                        canvases.push(cv);
                    } else {
                        self.recover_to_item();
                    }
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Sketch") => {
                    if let Some(sk) = self.parse_sketch() {
                        sketches.push(sk);
                    } else {
                        self.recover_to_item();
                    }
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Screen") => {
                    if let Some(sc) = self.parse_screen() {
                        screens.push(sc);
                    } else {
                        self.recover_to_item();
                    }
                }
                Tok::Ident(w) if Self::godot_base(w) => {
                    if let Some(n) = self.parse_godot_node() {
                        godot_nodes.push(n);
                    } else {
                        self.recover_to_item();
                    }
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Test") => {
                    match self.parse_test() {
                        Some(t) => tests.push(t),
                        None => self.recover_to_item(),
                    }
                }
                Tok::Ident(w) if w == "Option" => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        "`Option` directives (Option Base, Option Explicit, …) aren't \
                         supported and aren't needed — Rust is always zero-indexed and \
                         always explicit about types.",
                    );
                    self.recover_to_eol();
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Top level may only contain functions, found {:?}. \
                             Every Bust program starts at `Function Main()`.",
                            other
                        ),
                    );
                    // One error for the stray line, then resync — don't repeat
                    // it for every line of whatever this block turns out to be.
                    self.advance();
                    self.recover_to_item();
                }
            }
        }
        Program {
            leading_comments: top_comments,
            uses,
            constants,
            structs,
            enums,
            functions,
            windows,
            canvases,
            sketches,
            screens,
            godot_nodes,
            pages,
            css,
            tests,
        }
    }

    /// The Godot base classes a top-level block may open (`Node2D "Player" …`).
    /// Kept as one list so the dispatch and additions stay in one place. Emission
    /// is dimension-agnostic (the property/method passthrough doesn't care), so
    /// the 3D bases need only be named here.
    fn godot_base(w: &str) -> bool {
        matches!(
            w,
            // 2D
            "Node2D" | "CharacterBody2D" | "Area2D" | "Sprite2D" | "RigidBody2D" | "StaticBody2D"
            // 3D
            | "Node3D" | "CharacterBody3D" | "Area3D" | "MeshInstance3D" | "Camera3D"
            | "RigidBody3D" | "StaticBody3D"
            // dimensionless
            | "Node"
        )
    }

    /// `Use <crate> <version> [As <name>]` — the line was captured raw by the
    /// lexer; split it into the crate name, its version, and an optional import
    /// alias (`As PIL`, for a pip package imported under a different name).
    fn parse_use(&mut self) -> Option<UseDecl> {
        let line = self.line();
        let raw = match self.advance() {
            Tok::Use(s) => s,
            _ => return None,
        };
        let mut parts = raw.split_whitespace();
        let crate_name = match parts.next() {
            Some(c) => c.to_string(),
            None => {
                self.diags.error(line, "`Use` needs a crate name, e.g. `Use rand 0.8`.");
                return None;
            }
        };
        // Everything up to an `As` is the version; the single word after it is
        // the import alias (Python target).
        let rest: Vec<&str> = parts.collect();
        let as_pos = rest.iter().position(|w| w.eq_ignore_ascii_case("as"));
        let (version_parts, alias) = match as_pos {
            Some(p) => {
                let alias = match rest.get(p + 1) {
                    Some(a) if rest.len() == p + 2 => Some(a.to_string()),
                    _ => {
                        self.diags.error(
                            line,
                            format!(
                                "`Use {} … As` needs exactly one import name, e.g. \
                                 `Use {} 10.0 As PIL` (the module Python imports).",
                                crate_name, crate_name
                            ),
                        );
                        return None;
                    }
                };
                (&rest[..p], alias)
            }
            None => (&rest[..], None),
        };
        let version = version_parts.join(" ");
        if version.is_empty() {
            self.diags.error(
                line,
                format!(
                    "`Use {}` needs a version, e.g. `Use {} 0.8`. An explicit version keeps \
                     builds reproducible.",
                    crate_name, crate_name
                ),
            );
            return None;
        }
        Some(UseDecl {
            crate_name,
            version,
            alias,
            line,
        })
    }

    fn parse_const(&mut self, public: bool) -> Option<ConstDef> {
        let line = self.line();
        self.expect(&Tok::Const, "")?;
        let name = self.expect_ident("for the constant")?;
        self.expect(&Tok::As, "after the constant name")?;
        let ty = self.parse_type()?;
        self.expect(&Tok::Eq, "in the constant definition")?;
        let value = self.parse_expr()?;
        Some(ConstDef {
            name,
            public,
            ty,
            value,
            line,
        })
    }

    fn parse_struct(&mut self, public: bool) -> Option<StructDef> {
        self.expect(&Tok::Type, "to start a struct")?;
        let name = self.expect_ident("for the struct")?;
        if matches!(self.peek(), Tok::Lt) {
            self.diags.error_at(
                self.span(),
                self.line(),
                "Generic types (`Type Pair<T>`) aren't supported — declare concrete \
                 field types, or define the generic type in a `.rs` module (real Rust) \
                 and use it from there.",
            );
            return None;
        }
        self.expect(&Tok::Newline, "after the struct name")?;

        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End) {
                break;
            }
            // [Public | Private] Name As Type
            let field_public = if self.eat(&Tok::Public) {
                true
            } else {
                self.eat(&Tok::Private);
                false
            };
            let fname = self.expect_ident("for the field")?;
            self.expect(&Tok::As, "after the field name")?;
            let ty = self.parse_decl_type()?;
            fields.push(Field {
                name: fname,
                public: field_public,
                ty,
            });
            if !matches!(self.peek(), Tok::Newline | Tok::Eof) && !matches!(self.peek(), Tok::End) {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!("Expected end of line after the field, found {:?}.", self.peek()),
                );
                return None;
            }
        }

        self.expect(&Tok::End, "to close the struct")?;
        self.expect(&Tok::Type, "after `End`")?;
        self.eat(&Tok::Newline);
        Some(StructDef {
            name,
            public,
            fields,
        })
    }

    fn parse_enum(&mut self, public: bool) -> Option<EnumDef> {
        self.expect(&Tok::Enum, "to start an enum")?;
        let name = self.expect_ident("for the enum")?;
        if matches!(self.peek(), Tok::Lt) {
            self.diags.error_at(
                self.span(),
                self.line(),
                "Generic enums (`Enum Maybe<T>`) aren't supported — for \"a value or \
                 nothing\" use the built-in `Option<T>`/`Result<T>`, give the variant a \
                 concrete payload, or define the generic enum in a `.rs` module (real \
                 Rust).",
            );
            return None;
        }
        self.expect(&Tok::Newline, "after the enum name")?;

        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End) {
                break;
            }
            // A variant name (PascalCase, kept as written), with an optional tuple
            // payload: `Circle(Double)`, `Move(Long, Long)`.
            let vname = self.expect_ident("for an enum variant")?;
            let mut payload = Vec::new();
            if self.eat(&Tok::LParen) {
                if !matches!(self.peek(), Tok::RParen) {
                    loop {
                        // Variant payloads may be any type: primitives, String,
                        // structs, `Vec<T>`, nested enums, etc. Derives are computed
                        // conservatively so the generated enum always compiles.
                        // (Directly-recursive payloads still need `Vec`/`Option`;
                        // auto-boxing is a future addition.)
                        payload.push(self.parse_decl_type()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RParen, "to close the variant payload")?;
            }
            variants.push(EnumVariant { name: vname, payload });
            if !matches!(self.peek(), Tok::Newline | Tok::Eof) && !matches!(self.peek(), Tok::End) {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!("Expected end of line after the variant, found {:?}.", self.peek()),
                );
                return None;
            }
        }

        self.expect(&Tok::End, "to close the enum")?;
        self.expect(&Tok::Enum, "after `End`")?;
        self.eat(&Tok::Newline);
        if variants.is_empty() {
            self.diags
                .error_at(self.span(), self.line(), "An enum needs at least one variant.");
            return None;
        }
        Some(EnumDef { name, public, variants })
    }

    /// `Test "description"` … `End Test` — an executable specification. The body
    /// is an ordinary statement block (Arrange-Act-`Assert`).
    fn parse_test(&mut self) -> Option<TestBlock> {
        let line = self.line();
        self.expect_ident("to start a test")?; // the `Test` word
        let description = match self.advance() {
            Tok::Str(s) => s,
            other => {
                self.diags.error(
                    line,
                    format!(
                        "A `Test` needs a description in quotes — the sentence it verifies, \
                         e.g. `Test \"a blinker oscillates\"`. Found {:?}.",
                        other
                    ),
                );
                return None;
            }
        };
        self.expect(&Tok::Newline, "after the test description")?;
        let body = self.parse_block()?;
        self.expect(&Tok::End, "to close the test")?;
        // `End Test` — the terminator word is an identifier, like `Test` itself.
        match self.peek().clone() {
            Tok::Ident(w) if w.eq_ignore_ascii_case("Test") => {
                self.advance();
            }
            other => {
                self.diags.error_at(self.span(), self.line(), format!("Expected `Test` after `End`, found {:?}.", other));
                return None;
            }
        }
        self.eat(&Tok::Newline);
        Some(TestBlock { description, body, line })
    }

    fn parse_function(&mut self, public: bool, is_sub: bool, gpu: bool) -> Option<Function> {
        let line = self.line();
        if is_sub {
            self.expect(&Tok::Sub, "to start a sub")?;
            self.diags.warn_once(
                "sub-is-function",
                line,
                "`Sub` works, but in Bust it's just a `Function` with no return value — both \
                 become a Rust `fn`. You can write `Function` everywhere if you prefer.",
            );
        } else {
            self.expect(&Tok::Function, "to start a function")?;
        }
        let first = self.expect_ident(if is_sub { "for the sub" } else { "for the function" })?;
        // `Function Struct.Method()` is a method; otherwise a free function.
        let (receiver, name) = if self.eat(&Tok::Dot) {
            (Some(first), self.expect_ident("for the method name")?)
        } else {
            (None, first)
        };
        if matches!(self.peek(), Tok::Lt) {
            self.diags.error_at(
                self.span(),
                self.line(),
                "Generic functions (`Function Largest<T>(…)`) aren't supported. A useful \
                 generic needs trait bounds (`T: PartialOrd` to compare, `T: Clone` to \
                 copy, …), and those have no honest VB spelling — this is a moment to \
                 write real Rust. Put the function in a `.rs` file in your project and \
                 call it with the qualified form (`Utils.Largest(xs)`).",
            );
            return None;
        }
        self.expect(&Tok::LParen, "after the function name")?;

        let mut params = Vec::new();
        if !matches!(self.peek(), Tok::RParen) {
            loop {
                params.push(self.parse_param()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.expect(&Tok::RParen, "to close the parameter list")?;

        // A Sub never returns a value; a Function may declare a return type.
        let ret = if is_sub {
            if matches!(self.peek(), Tok::As) {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "A `Sub` returns nothing — to return a value, use `Function … As T`.",
                );
                return None;
            }
            None
        } else if self.eat(&Tok::As) {
            // `Function Foo() As Long` / `As Option<String>`. `As Result<T>` is
            // rejected — fallibility is implicit (see VBR_Errors_2_Final.md).
            let t = self.parse_decl_type()?;
            if let DeclType::Result(inner, _) = t {
                self.diags.error(
                    line,
                    "Functions don't declare `As Result<T>`. Fallibility is implicit: write \
                     `As Integer` (the success type), `RaiseError \"…\"` to fail, and \
                     `Handle err` at a call site to intercept. `Raw F()` yields the \
                     underlying `Result` as a value.",
                );
                Some(*inner)
            } else {
                Some(t)
            }
        } else {
            None
        };

        self.expect(&Tok::Newline, "after the header")?;
        let body = self.parse_block()?;
        self.expect(&Tok::End, if is_sub { "to close the sub" } else { "to close the function" })?;
        self.expect(if is_sub { &Tok::Sub } else { &Tok::Function }, "after `End`")?;
        // trailing newline is optional (EOF is fine)
        self.eat(&Tok::Newline);

        Some(Function {
            name,
            public,
            receiver,
            params,
            ret,
            body,
            line,
            gpu,
        })
    }

    // ── GUI (slice 1): Window / State / View / Event ──────────────────────────

    /// Expect an identifier equal to `name` (case-insensitive) — used for the
    /// GUI block keywords, which are contextual identifiers, not lexer keywords.
    fn expect_kw_ident(&mut self, name: &str) -> Option<()> {
        match self.advance() {
            Tok::Ident(w) if w.eq_ignore_ascii_case(name) => Some(()),
            other => {
                self.diags
                    .error_at(self.span(), self.line(), format!("Expected `{}`, found {:?}.", name, other));
                None
            }
        }
    }

    /// Expect a string literal.
    fn expect_string(&mut self, ctx: &str) -> Option<String> {
        match self.advance() {
            Tok::Str(s) => Some(s),
            other => {
                self.diags
                    .error_at(self.span(), self.line(), format!("Expected a string {}, found {:?}.", ctx, other));
                None
            }
        }
    }

    /// `Window Name … End Window` — or, with `kind = "Page"`, the identical
    /// block shape for a web page (same AST struct, different renderer).
    fn parse_window(&mut self, kind: &'static str) -> Option<Window> {
        self.advance(); // `Window` / `Page`
        let name = self.expect_ident("for the window name")?;
        self.expect(&Tok::Newline, "after the window name")?;

        let mut title = None;
        let mut theme = None;
        let mut state = Vec::new();
        let mut view = None;
        let mut events = Vec::new();
        let mut subs = Vec::new();

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident(kind)?;
                    self.eat(&Tok::Newline);
                    break;
                }
                // A `'` comment between members documents the next one (an
                // Event, say) — fine anywhere; not carried into the output.
                Tok::Comment(_) => {
                    self.advance();
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Title") => {
                    self.advance();
                    title = Some(self.expect_string("after `Title`")?);
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Theme") => {
                    self.advance();
                    theme = Some(self.expect_ident("for the theme name, e.g. `Theme Dracula`")?);
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("State") => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `State`")?;
                    state = self.parse_state_block()?;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("View") => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `View`")?;
                    view = Some(self.parse_view_node()?);
                    self.skip_newlines();
                    self.expect(&Tok::End, "to close `View`")?;
                    self.expect_kw_ident("View")?;
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Menu") => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "A Menu bar is Screen-only (terminal chrome). Put it on a Screen next to \
                             View, not in a {kind}."
                        ),
                    );
                    return None;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Event") => {
                    self.advance();
                    let ev_name = self.expect_ident("for the event name")?;
                    // Optional payload params: `Event Rename(value As String)`.
                    let params = if self.eat(&Tok::LParen) {
                        self.parse_params_until_rparen()?
                    } else {
                        Vec::new()
                    };
                    self.expect(&Tok::Newline, "after the event name")?;
                    let body = self.parse_block()?;
                    self.expect(&Tok::End, "to close the event")?;
                    self.expect_kw_ident("Event")?;
                    self.eat(&Tok::Newline);
                    events.push(GuiEvent { name: ev_name, params, body });
                }
                Tok::Sub => {
                    subs.push(self.parse_gui_sub()?);
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Unexpected {:?} inside a {kind} — expected Title, Theme, State, \
                             View, Event, Sub, or `End {kind}`.",
                            other
                        ),
                    );
                    return None;
                }
            }
        }

        let view = match view {
            Some(v) => v,
            None => {
                self.diags
                    .error_at(self.span(), self.line(), format!("A {kind} needs a `View` block."));
                return None;
            }
        };
        Some(Window {
            name,
            title,
            theme,
            state,
            view,
            events,
            subs,
        })
    }

    /// A `Sub Name(params) … End Sub` inside a Window/Screen/Page — a helper that
    /// lowers to a method on the state (direct field access, callable from events
    /// and other helpers). Same shape as an event, so it reuses `GuiEvent`.
    fn parse_gui_sub(&mut self) -> Option<GuiEvent> {
        self.expect(&Tok::Sub, "to start a Sub")?;
        let name = self.expect_ident("for the Sub name")?;
        // Full function-style params (`ByVal`/`ByRef`, defaults), not the
        // by-value-only event payload form — a helper reads like a `Sub`.
        let mut params = Vec::new();
        if self.eat(&Tok::LParen) {
            if !matches!(self.peek(), Tok::RParen) {
                loop {
                    params.push(self.parse_param()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
            }
            self.expect(&Tok::RParen, "to close the parameter list")?;
        }
        self.expect(&Tok::Newline, "after the Sub name")?;
        let body = self.parse_block()?;
        self.expect(&Tok::End, "to close the Sub")?;
        self.expect(&Tok::Sub, "after `End`")?;
        self.eat(&Tok::Newline);
        Some(GuiEvent { name, params, body })
    }

    /// `Screen Name` … `End Screen` — a ratatui terminal app. Same State/View/
    /// Event blocks as a Window, but events are bound by a keymap: `On Key "q" Quit`.
    fn parse_screen(&mut self) -> Option<Screen> {
        self.advance(); // `Screen`
        let name = self.expect_ident("for the screen name")?;
        self.expect(&Tok::Newline, "after the screen name")?;

        let mut title = None;
        let mut theme = None;
        let mut status = None;
        let mut menu = None;
        let mut state = Vec::new();
        let mut view = None;
        let mut keys = Vec::new();
        let mut timers = Vec::new();
        let mut events = Vec::new();
        let mut subs = Vec::new();

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident("Screen")?;
                    self.eat(&Tok::Newline);
                    break;
                }
                // A `'` comment between members documents the next one (an
                // Event, say) — fine anywhere; not carried into the output.
                Tok::Comment(_) => {
                    self.advance();
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Title") => {
                    self.advance();
                    title = Some(self.expect_string("after `Title`")?);
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Theme") => {
                    self.advance();
                    theme = Some(self.expect_ident("for the theme name, e.g. `Theme NightOwl`")?);
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Status") => {
                    // `Status <expr>` — left side of the bottom hotkey bar.
                    self.advance();
                    status = Some(self.parse_expr()?);
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Menu") => {
                    // Screen chrome — a top bar of dropdowns, sibling to View.
                    if menu.is_some() {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            "A Screen has one `Menu` block (the bar). Put more dropdowns inside it.",
                        );
                        return None;
                    }
                    self.advance();
                    menu = Some(self.parse_screen_menu()?);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("State") => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `State`")?;
                    state = self.parse_state_block()?;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("View") => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `View`")?;
                    view = Some(self.parse_view_node()?);
                    self.skip_newlines();
                    self.expect(&Tok::End, "to close `View`")?;
                    self.expect_kw_ident("View")?;
                    self.eat(&Tok::Newline);
                }
                // `On Key "q" Handler` — a keymap binding.
                Tok::On => {
                    self.advance();
                    self.expect_kw_ident("Key")?;
                    let key = self.parse_key_spec()?;
                    let handler = self.expect_ident("for the key's handler event")?;
                    let label = if let Tok::Str(s) = self.peek().clone() {
                        self.advance();
                        Some(s)
                    } else {
                        None
                    };
                    self.eat(&Tok::Newline);
                    keys.push(KeyBinding { key, handler, label });
                }
                // `Every 1000 Handler` — a timer binding (interval in ms).
                Tok::Ident(w) if w.eq_ignore_ascii_case("Every") => {
                    self.advance();
                    let interval_ms = self.parse_array_size()? as u64;
                    let handler = self.expect_ident("for the timer's handler event")?;
                    self.eat(&Tok::Newline);
                    timers.push(Timer { interval_ms, handler });
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Event") => {
                    self.advance();
                    let ev_name = self.expect_ident("for the event name")?;
                    let params = if self.eat(&Tok::LParen) {
                        self.parse_params_until_rparen()?
                    } else {
                        Vec::new()
                    };
                    self.expect(&Tok::Newline, "after the event name")?;
                    let body = self.parse_block()?;
                    self.expect(&Tok::End, "to close the event")?;
                    self.expect_kw_ident("Event")?;
                    self.eat(&Tok::Newline);
                    events.push(GuiEvent { name: ev_name, params, body });
                }
                Tok::Sub => {
                    subs.push(self.parse_gui_sub()?);
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Unexpected {:?} inside a Screen — expected Title, Theme, Status, Menu, State, View, \
                             `On Key`, `Every`, Event, Sub, or `End Screen`.",
                            other
                        ),
                    );
                    return None;
                }
            }
        }

        let view = match view {
            Some(v) => v,
            None => {
                self.diags.error_at(self.span(), self.line(), "A Screen needs a `View` block.");
                return None;
            }
        };
        Some(Screen { name, title, theme, status, menu, state, view, keys, timers, events, subs })
    }

    /// `Menu` … `Menu "File"` … `Item "Quit" Quit` … `End Menu` … `End Menu`.
    /// The opening `Menu` token has already been consumed.
    fn parse_screen_menu(&mut self) -> Option<ScreenMenu> {
        self.eat(&Tok::Newline);
        let mut menus = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident("Menu")?;
                    self.eat(&Tok::Newline);
                    break;
                }
                Tok::Comment(_) => {
                    self.advance();
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Menu") => {
                    self.advance();
                    menus.push(self.parse_menu_group()?);
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Inside `Menu` expected `Menu \"title\"` or `End Menu`, found {:?}.",
                            other
                        ),
                    );
                    return None;
                }
            }
        }
        if menus.is_empty() {
            self.diags.error_at(
                self.span(),
                self.line(),
                "A Menu bar needs at least one dropdown — `Menu \"File\"` … `End Menu`.",
            );
            return None;
        }
        Some(ScreenMenu { menus })
    }

    /// `Menu "File"` … `Item` / `Separator` … `End Menu`. Opening `Menu` eaten.
    fn parse_menu_group(&mut self) -> Option<MenuGroup> {
        let title = self.expect_string("for the menu title — `Menu \"File\"`")?;
        self.eat(&Tok::Newline);
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident("Menu")?;
                    self.eat(&Tok::Newline);
                    break;
                }
                Tok::Comment(_) => {
                    self.advance();
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Item") => {
                    self.advance();
                    let label = self.expect_string("for the item label — `Item \"Quit\" Quit`")?;
                    let handler = self.expect_ident("for the item's event (or `Quit`)")?;
                    self.eat(&Tok::Newline);
                    items.push(MenuEntry::Item { label, handler });
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Separator") => {
                    self.advance();
                    self.eat(&Tok::Newline);
                    items.push(MenuEntry::Separator);
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Inside a dropdown expected `Item \"label\" Event`, `Separator`, or \
                             `End Menu`, found {:?}.",
                            other
                        ),
                    );
                    return None;
                }
            }
        }
        if !items.iter().any(|i| matches!(i, MenuEntry::Item { .. })) {
            self.diags.error_at(
                self.span(),
                self.line(),
                format!(
                    "Menu \"{}\" needs at least one `Item \"label\" Event`.",
                    title
                ),
            );
            return None;
        }
        Some(MenuGroup { title, items })
    }

    /// `Node2D "Player" … End Node2D` — a Godot node class. The opening token is
    /// the base class (`Node2D`); the name is a string (the registered class /
    /// Rust struct). Members are `Export`/`Dim` fields; `On Ready` / `On
    /// Process(delta)` are lifecycle callbacks. (Godot slice 1.)
    fn parse_godot_node(&mut self) -> Option<GodotNode> {
        let line = self.line();
        let base = match self.advance() {
            Tok::Ident(w) => w,
            other => {
                self.diags.error_at(
                    self.prev_span(),
                    line,
                    format!("Expected a Godot base class, found {:?}.", other),
                );
                return None;
            }
        };
        let name = self.expect_string("for the node's class name (e.g. `Node2D \"Player\"`)")?;
        self.expect(&Tok::Newline, "after the node name")?;

        let mut fields = Vec::new();
        let mut events = Vec::new();
        let mut signals = Vec::new();
        let mut handlers = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident(&base)?;
                    self.eat(&Tok::Newline);
                    break;
                }
                Tok::Comment(_) => {
                    self.advance();
                    self.eat(&Tok::Newline);
                }
                // `Signal Died` / `Signal ScoreChanged(points As Long)`.
                Tok::Ident(w) if w.eq_ignore_ascii_case("Signal") => {
                    self.advance();
                    let sig_name = self.expect_ident("for the signal name")?;
                    let params = if self.eat(&Tok::LParen) {
                        self.parse_params_until_rparen()?
                    } else {
                        Vec::new()
                    };
                    self.eat(&Tok::Newline);
                    signals.push(GodotSignal { name: sig_name, params });
                }
                // `Export Speed As Single = 200` / `Dim Health As Long = 100` —
                // a member field. `Export` surfaces it in the Godot inspector.
                Tok::Ident(w) if w.eq_ignore_ascii_case("Export") => {
                    self.advance();
                    fields.push(self.parse_godot_field(true)?);
                }
                Tok::Dim => {
                    self.advance();
                    fields.push(self.parse_godot_field(false)?);
                }
                // `Sub OnPinged(count As Long) … End Sub` — a signal handler
                // (a callable `#[func]`), wired up with `Connect … To`.
                Tok::Sub => {
                    self.advance();
                    let h_name = self.expect_ident("for the handler name")?;
                    let params = if self.eat(&Tok::LParen) {
                        self.parse_params_until_rparen()?
                    } else {
                        Vec::new()
                    };
                    self.expect(&Tok::Newline, "after the handler name")?;
                    let body = self.parse_block()?;
                    self.expect(&Tok::End, "to close the handler")?;
                    self.expect(&Tok::Sub, "after `End`")?;
                    self.eat(&Tok::Newline);
                    handlers.push(GodotEvent { name: h_name, params, body });
                }
                // `On Ready … End Ready` / `On Process(delta) … End Process`.
                Tok::On => {
                    self.advance();
                    let ev_name = self.expect_ident("for the event name (e.g. `On Process`)")?;
                    let params = if self.eat(&Tok::LParen) {
                        self.parse_godot_event_params()?
                    } else {
                        Vec::new()
                    };
                    self.expect(&Tok::Newline, "after the event name")?;
                    let body = self.parse_block()?;
                    self.expect(&Tok::End, "to close the event")?;
                    self.expect_kw_ident(&ev_name)?;
                    self.eat(&Tok::Newline);
                    events.push(GodotEvent { name: ev_name, params, body });
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Unexpected {:?} inside a {} — expected `Export`, `Dim`, \
                             `On <Event>`, or `End {}`.",
                            other, base, base
                        ),
                    );
                    return None;
                }
            }
        }
        Some(GodotNode { base, name, fields, events, signals, handlers, line })
    }

    /// A Godot event's parameter list (the `(` is already eaten). A lifecycle
    /// callback's built-in argument may be written untyped (`On Process(delta)`) —
    /// its type is known from the event, so it defaults to `Single` (delta is
    /// seconds since the last frame). An explicit `As <Type>` is also accepted.
    fn parse_godot_event_params(&mut self) -> Option<Vec<Param>> {
        let mut params = Vec::new();
        if !matches!(self.peek(), Tok::RParen) {
            loop {
                let name = self.expect_ident("for the parameter")?;
                let ty = if self.eat(&Tok::As) {
                    self.parse_decl_type()?
                } else {
                    DeclType::Plain(Type::Single)
                };
                params.push(Param { name, ty, mode: ParamMode::ByVal });
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.expect(&Tok::RParen, "to close the parameter list")?;
        Some(params)
    }

    /// One `[Export] <Name> As <Type> [= <default>]` member of a Godot node.
    fn parse_godot_field(&mut self, export: bool) -> Option<GodotField> {
        let name = self.expect_ident("for the field name")?;
        self.expect(&Tok::As, "after the field name")?;
        let ty = self.parse_decl_type()?;
        let default = if self.eat(&Tok::Eq) { Some(self.parse_expr()?) } else { None };
        self.eat(&Tok::Newline);
        Some(GodotField { name, ty, default, export })
    }

    /// A key spec after `On Key`: a string literal for a character (`"q"`, `"+"`)
    /// or an identifier for a named key (`Up`, `Down`, `Enter`, `Esc`, `Tab`).
    fn parse_key_spec(&mut self) -> Option<String> {
        match self.peek().clone() {
            Tok::Str(s) => {
                self.advance();
                Some(s)
            }
            Tok::Ident(name) => {
                self.advance();
                Some(name)
            }
            other => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!(
                        "Expected a key after `On Key` — a character like \"q\" or a named key \
                         (Up, Down, Enter, Esc), found {:?}.",
                        other
                    ),
                );
                None
            }
        }
    }

    /// `Sketch Name` … `End Sketch` — a pixel window that is the drawing.
    fn parse_sketch(&mut self) -> Option<Sketch> {
        self.advance(); // `Sketch`
        let name = self.expect_ident("for the sketch name")?;
        self.expect(&Tok::Newline, "after the sketch name")?;

        let mut title = None;
        let mut width = 800u32;
        let mut height = 600u32;
        let mut background = None;
        let mut state = Vec::new();
        let mut timers = Vec::new();
        let mut gpu_draw = None;
        let mut draw = None;
        let mut events = Vec::new();
        let mut subs = Vec::new();

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident("Sketch")?;
                    self.eat(&Tok::Newline);
                    break;
                }
                Tok::Comment(_) => {
                    self.advance();
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Title") => {
                    self.advance();
                    title = Some(self.expect_string("after `Title`")?);
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Size") => {
                    self.advance();
                    width = self.parse_pixel_size()?;
                    self.expect(&Tok::Comma, "between width and height — `Size 800, 600`")?;
                    height = self.parse_pixel_size()?;
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Background") => {
                    self.advance();
                    background = Some(self.parse_expr()?);
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Theme") => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        "A Sketch is coloured with `Background Color.Navy`, not Theme. \
                         Theme restyles a Window, Screen, or Page.",
                    );
                    return None;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("View") => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        "A Sketch has no View — it *is* the drawing. Put shapes in a `Draw` \
                         block. For buttons and sliders, use a `Window` with a `Canvas`.",
                    );
                    return None;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("State") => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `State`")?;
                    state = self.parse_state_block()?;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Gpu") => {
                    self.advance();
                    self.expect_kw_ident("Draw")?;
                    self.expect(&Tok::Newline, "after `Gpu Draw`")?;
                    gpu_draw = Some(self.parse_block()?);
                    self.expect(&Tok::End, "to close `Gpu Draw`")?;
                    self.expect_kw_ident("Draw")?;
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Draw") => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `Draw`")?;
                    draw = Some(self.parse_block()?);
                    self.expect(&Tok::End, "to close `Draw`")?;
                    self.expect_kw_ident("Draw")?;
                    self.eat(&Tok::Newline);
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Every") => {
                    self.advance();
                    let interval_ms = self.parse_array_size()? as u64;
                    let handler = self.expect_ident("for the timer's handler event")?;
                    self.eat(&Tok::Newline);
                    timers.push(Timer { interval_ms, handler });
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Event") => {
                    self.advance();
                    let ev_name = self.expect_ident("for the event name")?;
                    let params = if self.eat(&Tok::LParen) {
                        self.parse_params_until_rparen()?
                    } else {
                        Vec::new()
                    };
                    self.expect(&Tok::Newline, "after the event name")?;
                    let body = self.parse_block()?;
                    self.expect(&Tok::End, "to close the event")?;
                    self.expect_kw_ident("Event")?;
                    self.eat(&Tok::Newline);
                    events.push(GuiEvent { name: ev_name, params, body });
                }
                Tok::Sub => {
                    subs.push(self.parse_gui_sub()?);
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Unexpected {:?} inside a Sketch — expected Title, Size, Background, \
                             State, Draw, Gpu Draw, Every, Event, Sub, or `End Sketch`.",
                            other
                        ),
                    );
                    return None;
                }
            }
        }

        let draw = draw.unwrap_or_default();
        if gpu_draw.is_none() && draw.is_empty() {
            self.diags.error_at(
                self.span(),
                self.line(),
                "A Sketch needs a `Draw` block or a `Gpu Draw` block.",
            );
            return None;
        }
        Some(Sketch {
            name,
            title,
            width,
            height,
            background,
            state,
            timers,
            gpu_draw,
            draw,
            events,
            subs,
        })
    }

    fn parse_pixel_size(&mut self) -> Option<u32> {
        if let Tok::Int(n) = self.peek() {
            let n = *n;
            self.advance();
            if n <= 0 {
                self.diags.error_at(
                    self.prev_span(),
                    self.line(),
                    "A Size must be a positive pixel count, e.g. `Size 800, 600`.",
                );
                return None;
            }
            Some(n as u32)
        } else {
            self.diags.error_at(
                self.span(),
                self.line(),
                "A Size must be a positive pixel count, e.g. `Size 800, 600`.",
            );
            None
        }
    }

    /// `Canvas Name` … `Draw` … `End Draw` … `End Canvas` — a drawing surface.
    fn parse_canvas(&mut self) -> Option<CanvasDef> {
        self.advance(); // `Canvas`
        let name = self.expect_ident("for the canvas name")?;
        self.expect(&Tok::Newline, "after the canvas name")?;

        let mut body = None;
        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident("Canvas")?;
                    self.eat(&Tok::Newline);
                    break;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Draw") => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `Draw`")?;
                    body = Some(self.parse_block()?);
                    self.expect(&Tok::End, "to close `Draw`")?;
                    self.expect_kw_ident("Draw")?;
                    self.eat(&Tok::Newline);
                }
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!(
                            "Inside a Canvas expected a `Draw` block or `End Canvas`, found {:?}.",
                            other
                        ),
                    );
                    return None;
                }
            }
        }

        let body = match body {
            Some(b) => b,
            None => {
                self.diags
                    .error_at(self.span(), self.line(), "A Canvas needs a `Draw` block.");
                return None;
            }
        };
        Some(CanvasDef { name, body })
    }

    /// Peek the token one past the cursor (for small look-ahead decisions).
    fn peek2(&self) -> &Tok {
        let i = (self.pos + 1).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn peek3(&self) -> &Tok {
        let i = (self.pos + 2).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    /// A drawing verb statement: `Fill`/`Stroke`/`Text` followed by its operands.
    /// Valid inside a `Draw` block or a paint function; the AST is shared, and the
    /// canvas codegen threads the `frame` through.
    fn parse_draw_cmd(&mut self, verb: &str) -> Option<Stmt> {
        self.advance(); // the verb ident
        let cmd = match verb.to_ascii_lowercase().as_str() {
            "fill" => {
                let shape = self.parse_shape()?;
                self.expect(&Tok::Comma, "after the shape — `Fill <shape>, <color>`")?;
                let color = self.parse_expr()?;
                DrawCmd::Fill { shape, color }
            }
            "stroke" => {
                let shape = self.parse_shape()?;
                self.expect(&Tok::Comma, "after the shape — `Stroke <shape>, <color>`")?;
                let color = self.parse_expr()?;
                let width = if self.eat(&Tok::Comma) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                DrawCmd::Stroke { shape, color, width }
            }
            "text" => {
                let text = self.parse_expr()?;
                self.expect(&Tok::Comma, "after the text — `Text <string>, <x>, <y>`")?;
                let x = self.parse_expr()?;
                self.expect(&Tok::Comma, "between x and y — `Text <string>, <x>, <y>`")?;
                let y = self.parse_expr()?;
                let color = if self.eat(&Tok::Comma) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                DrawCmd::Text { text, x, y, color }
            }
            "pixel" => {
                let x = self.parse_expr()?;
                self.expect(&Tok::Comma, "after x — `Pixel <x>, <y>, <color>`")?;
                let y = self.parse_expr()?;
                self.expect(&Tok::Comma, "after y — `Pixel <x>, <y>, <color>`")?;
                let color = self.parse_expr()?;
                DrawCmd::Pixel { x, y, color }
            }
            "clear" => {
                let color = self.parse_expr()?;
                DrawCmd::Clear { color }
            }
            "copy" => self.parse_copy_cmd()?,
            _ => unreachable!(),
        };
        Some(Stmt::Draw(cmd))
    }

    /// `Copy spr, dx, dy` / `Copy spr, dx, dy, dw, dh` / `Copy spr, dx, dy, sx, sy, w, h`
    /// then optional `Using mask`, `ColorKey, color`, `Blend Add|Multiply`.
    fn parse_copy_cmd(&mut self) -> Option<DrawCmd> {
        let src = self.expect_ident("for the buffer to Copy — a `Pixels` name, or `frame`")?;
        let mut args = Vec::new();
        let mut mask = None;
        let mut color_key = None;
        let mut blend = GpuBlend::Replace;
        while self.eat(&Tok::Comma) {
            if let Tok::Ident(w) = self.peek().clone() {
                let lw = w.to_ascii_lowercase();
                if lw == "using" || lw == "colorkey" || lw == "blend" {
                    self.parse_copy_clause(&mut mask, &mut color_key, &mut blend)?;
                    while self.eat(&Tok::Comma) {
                        self.parse_copy_clause(&mut mask, &mut color_key, &mut blend)?;
                    }
                    break;
                }
            }
            args.push(self.parse_expr()?);
        }
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            self.diags.error_at(
                self.span(),
                self.line(),
                "`Copy` is `Copy spr, x, y` or `Copy spr, dx, dy, dw, dh` or \
                 `Copy spr, dx, dy, sx, sy, w, h`.",
            );
            return None;
        }
        Some(DrawCmd::Copy {
            src,
            args,
            mask,
            color_key,
            blend,
        })
    }

    fn parse_copy_clause(
        &mut self,
        mask: &mut Option<String>,
        color_key: &mut Option<Expr>,
        blend: &mut GpuBlend,
    ) -> Option<()> {
        let w = self.expect_ident("for `Using`, `ColorKey`, or `Blend`")?;
        match w.to_ascii_lowercase().as_str() {
            "using" => {
                *mask = Some(self.expect_ident("for the mask `Pixels`")?);
            }
            "colorkey" => {
                self.eat(&Tok::Comma);
                *color_key = Some(self.parse_expr()?);
            }
            "blend" => {
                let mode = self.expect_ident("for `Blend Add` or `Blend Multiply`")?;
                *blend = match mode.to_ascii_lowercase().as_str() {
                    "add" => GpuBlend::Add,
                    "multiply" => GpuBlend::Multiply,
                    other => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            format!("`Blend {other}` isn't GPU-legal — use `Add` or `Multiply`."),
                        );
                        return None;
                    }
                };
            }
            other => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!("Unexpected `{other}` after `Copy` — expected Using, ColorKey, or Blend."),
                );
                return None;
            }
        }
        Some(())
    }

    /// A drawing shape: `Circle(cx, cy, r)`, `Rect(x, y, w, h)`, `Line(x1, y1, x2, y2)`.
    fn parse_shape(&mut self) -> Option<Shape> {
        let kind = self.expect_ident("for the shape — Circle, Rect, or Line")?;
        self.expect(&Tok::LParen, "after the shape name")?;
        let mut args = vec![self.parse_expr()?];
        while self.eat(&Tok::Comma) {
            args.push(self.parse_expr()?);
        }
        self.expect(&Tok::RParen, "to close the shape")?;
        let mut it = args.into_iter();
        macro_rules! next_arg {
            ($what:literal) => {
                match it.next() {
                    Some(e) => e,
                    None => {
                        self.diags.error_at(self.span(), self.line(), $what);
                        return None;
                    }
                }
            };
        }
        let shape = match kind.to_ascii_lowercase().as_str() {
            "circle" => Shape::Circle(
                next_arg!("Circle needs (cx, cy, radius)."),
                next_arg!("Circle needs (cx, cy, radius)."),
                next_arg!("Circle needs (cx, cy, radius)."),
            ),
            "rect" => Shape::Rect(
                next_arg!("Rect needs (x, y, width, height)."),
                next_arg!("Rect needs (x, y, width, height)."),
                next_arg!("Rect needs (x, y, width, height)."),
                next_arg!("Rect needs (x, y, width, height)."),
            ),
            "line" => Shape::Line(
                next_arg!("Line needs (x1, y1, x2, y2)."),
                next_arg!("Line needs (x1, y1, x2, y2)."),
                next_arg!("Line needs (x1, y1, x2, y2)."),
                next_arg!("Line needs (x1, y1, x2, y2)."),
            ),
            other => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!("Unknown shape `{}` — use Circle, Rect, or Line.", other),
                );
                return None;
            }
        };
        if it.next().is_some() {
            self.diags
                .error_at(self.span(), self.line(), "Too many arguments for this shape.");
            return None;
        }
        Some(shape)
    }

    fn parse_state_block(&mut self) -> Option<Vec<StateField>> {
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End) {
                self.advance();
                self.expect_kw_ident("State")?;
                self.eat(&Tok::Newline);
                break;
            }
            // A comment line between fields is fine, as anywhere else.
            if matches!(self.peek(), Tok::Comment(_)) {
                self.advance();
                self.eat(&Tok::Newline);
                continue;
            }
            if !matches!(self.peek(), Tok::Dim) {
                self.diags
                    .error_at(self.span(), self.line(), "A `State` block may only contain `Dim` declarations.");
                return None;
            }
            // One line may declare several fields (`Dim x As Integer = 0, y As Integer = 0`).
            let first = self.parse_dim()?;
            let mut dims = vec![first];
            dims.append(&mut self.dim_overflow);
            for d in dims {
                match d {
                    // A primitive or user enum needs an initial value.
                    Stmt::Dim {
                        name,
                        ty: ty @ (DeclType::Plain(_) | DeclType::Named(_)),
                        init: Some(init),
                        ..
                    } => fields.push(StateField { name, ty, init: Some(init) }),
                    // A `Vec` collection may start empty (init optional) — the dynamic
                    // dataset behind charts/plots. (Map/fixed arrays can follow later.)
                    Stmt::Dim { name, ty: ty @ DeclType::Vec(_), init, .. } => {
                        fields.push(StateField { name, ty, init })
                    }
                    _ => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            "A State field must be a typed value with an initial value \
                             (`Dim count As Integer = 0`), or a collection that may start empty \
                             (`Dim data As Vec<Double>`).",
                        );
                        return None;
                    }
                }
            }
            self.eat(&Tok::Newline);
        }
        Some(fields)
    }

    fn parse_view_node(&mut self) -> Option<ViewNode> {
        self.skip_newlines();
        // `Match`/`If` lex to keyword tokens, so handle them before widget names.
        if matches!(self.peek(), Tok::Match) {
            return self.parse_view_match();
        }
        if matches!(self.peek(), Tok::If) {
            return self.parse_view_if();
        }
        let kw = match self.peek().clone() {
            Tok::Ident(w) => w,
            other => {
                self.diags
                    .error_at(self.span(), self.line(), format!("Expected a widget, found {:?}.", other));
                return None;
            }
        };
        match kw.to_ascii_lowercase().as_str() {
            "column" => {
                self.advance();
                self.eat(&Tok::Newline);
                let (children, spacing, padding) = self.parse_container_body("Column")?;
                Some(ViewNode::Column { children, spacing, padding })
            }
            "row" => {
                self.advance();
                self.eat(&Tok::Newline);
                let (children, spacing, padding) = self.parse_container_body("Row")?;
                Some(ViewNode::Row { children, spacing, padding })
            }
            "frame" => {
                // `Frame ["title"]` … `End Frame` — a bordered panel (Screen).
                self.advance();
                let title = if matches!(self.peek(), Tok::Newline) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.eat(&Tok::Newline);
                let (children, spacing, padding) = self.parse_container_body("Frame")?;
                Some(ViewNode::Frame {
                    title,
                    children,
                    spacing,
                    padding,
                })
            }
            "tabs" => {
                // `Tabs field` … `Tab "title"` … `End Tab` … `End Tabs`.
                self.advance();
                let field = self.expect_ident("for the Tabs index field")?;
                self.eat(&Tok::Newline);
                let mut on_change = None;
                let mut tabs = Vec::new();
                loop {
                    self.skip_newlines();
                    match self.peek().clone() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Change")?;
                            on_change = Some(self.expect_ident("for the change event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Tabs")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        Tok::Ident(w) if w.eq_ignore_ascii_case("tab") => {
                            self.advance();
                            let title = self.parse_expr()?;
                            self.eat(&Tok::Newline);
                            let (children, _, _) = self.parse_container_body("Tab")?;
                            tabs.push(TabPane { title, children });
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside Tabs expected `Tab <title>`, `On Change <event>`, or \
                                     `End Tabs`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                if tabs.is_empty() {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        "A Tabs widget needs at least one `Tab` pane.",
                    );
                    return None;
                }
                Some(ViewNode::Tabs {
                    field,
                    tabs,
                    on_change,
                })
            }
            "tab" => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "`Tab` belongs inside `Tabs` — wrap it: Tabs field / Tab \"title\" / … / \
                     End Tab / End Tabs.",
                );
                None
            }
            "menu" => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "A Menu belongs on the Screen next to View, not inside the View — \
                     Menu / Menu \"File\" / Item \"Quit\" Quit / End Menu / End Menu.",
                );
                None
            }
            "space" => {
                // `Space Height 20` / `Space Width 10` — a blank gap.
                self.advance();
                let dim = self.expect_ident("for `Space` — `Height` or `Width`")?;
                let horizontal = match dim.to_ascii_lowercase().as_str() {
                    "width" => true,
                    "height" => false,
                    _ => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            format!("`Space` takes `Height` or `Width`, found `{}`.", dim),
                        );
                        return None;
                    }
                };
                let amount = self.parse_array_size()? as u16;
                self.eat(&Tok::Newline);
                Some(ViewNode::Space { horizontal, amount })
            }
            "scrollable" => {
                self.advance();
                self.eat(&Tok::Newline);
                let (children, spacing, padding) = self.parse_container_body("Scrollable")?;
                Some(ViewNode::Scrollable { children, spacing, padding })
            }
            "rule" => {
                self.advance();
                let dim = self.expect_ident("for `Rule` — `Horizontal` or `Vertical`")?;
                let horizontal = match dim.to_ascii_lowercase().as_str() {
                    "horizontal" => true,
                    "vertical" => false,
                    _ => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            format!("`Rule` takes `Horizontal` or `Vertical`, found `{dim}`."),
                        );
                        return None;
                    }
                };
                self.eat(&Tok::Newline);
                Some(ViewNode::Rule { horizontal })
            }
            "chooser" => {
                self.advance();
                let value = self.expect_ident("for the bound Chooser field")?;
                let from = self.expect_ident("after the field — `Chooser field From options`")?;
                if !from.eq_ignore_ascii_case("from") {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!("A Chooser is `Chooser field From options` — found `{from}`."),
                    );
                    return None;
                }
                let options = self.expect_ident("for the options Vec field")?;
                self.eat(&Tok::Newline);
                let mut on_select = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            self.expect(&Tok::Select, "in `On Select`")?;
                            on_select = Some(self.expect_ident("for the select event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Chooser")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Chooser expected `On Select <event>` or `End Chooser`, \
                                     found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                let on_select = match on_select {
                    Some(ev) => ev,
                    None => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            "A Chooser needs `On Select <event>` — the pick list always reports \
                             the chosen value.",
                        );
                        return None;
                    }
                };
                Some(ViewNode::Chooser { value, options, on_select })
            }
            "markdown" => {
                self.advance();
                let source = self.parse_expr()?;
                self.eat(&Tok::Newline);
                Some(ViewNode::Markdown { source })
            }
            "svg" => {
                self.advance();
                let path = self.parse_expr()?;
                self.eat(&Tok::Newline);
                Some(ViewNode::Svg { path })
            }
            "text" => {
                self.advance();
                let e = self.parse_expr()?;
                self.eat(&Tok::Newline);
                Some(ViewNode::Text(e))
            }
            "image" => {
                self.advance();
                let path = self.parse_expr()?;
                self.eat(&Tok::Newline);
                Some(ViewNode::Image { path })
            }
            "gauge" => {
                // `Gauge min..=max, field` — a progress gauge (display-only).
                self.advance();
                let min = self.parse_expr()?;
                self.expect(&Tok::DotDotEq, "for the gauge range — `min..=max`")?;
                let max = self.parse_expr()?;
                self.expect(&Tok::Comma, "after the range — `Gauge min..=max, field`")?;
                let value = self.expect_ident("for the bound numeric field")?;
                self.eat(&Tok::Newline);
                Some(ViewNode::Gauge { min, max, value })
            }
            "sparkline" => {
                // `Sparkline field` — a trend line over a Vec of numbers.
                self.advance();
                let field = self.expect_ident("for the Sparkline's numeric Vec field")?;
                self.eat(&Tok::Newline);
                Some(ViewNode::Sparkline { field })
            }
            "barchart" => {
                // `BarChart field` — bars over a Vec of structs (label + value).
                self.advance();
                let field = self.expect_ident("for the BarChart's Vec<Struct> field")?;
                self.eat(&Tok::Newline);
                Some(ViewNode::BarChart { field })
            }
            "chart" => {
                // Single-line: `Chart f1[, f2, …] [Scatter]` (auto axes).
                // Block:       `Chart` / `Series f` / `XAxis min..=max` / … / `End Chart`.
                self.advance();
                let mut fields = Vec::new();
                let mut scatter = false;
                let mut x_bounds = None;
                let mut y_bounds = None;
                if matches!(self.peek(), Tok::Newline) {
                    self.advance();
                    loop {
                        self.skip_newlines();
                        match self.peek().clone() {
                            Tok::End => {
                                self.advance();
                                self.expect_kw_ident("Chart")?;
                                self.eat(&Tok::Newline);
                                break;
                            }
                            Tok::Ident(w) if w.eq_ignore_ascii_case("Series") => {
                                self.advance();
                                fields.push(self.expect_ident("for the series' Vec<Struct> field")?);
                                self.eat(&Tok::Newline);
                            }
                            Tok::Ident(w) if w.eq_ignore_ascii_case("Scatter") => {
                                self.advance();
                                scatter = true;
                                self.eat(&Tok::Newline);
                            }
                            Tok::Ident(w) if w.eq_ignore_ascii_case("XAxis") => {
                                self.advance();
                                let lo = self.parse_expr()?;
                                self.expect(&Tok::DotDotEq, "for the axis range — `min..=max`")?;
                                let hi = self.parse_expr()?;
                                self.eat(&Tok::Newline);
                                x_bounds = Some((lo, hi));
                            }
                            Tok::Ident(w) if w.eq_ignore_ascii_case("YAxis") => {
                                self.advance();
                                let lo = self.parse_expr()?;
                                self.expect(&Tok::DotDotEq, "for the axis range — `min..=max`")?;
                                let hi = self.parse_expr()?;
                                self.eat(&Tok::Newline);
                                y_bounds = Some((lo, hi));
                            }
                            other => {
                                self.diags.error_at(
                                    self.span(),
                                    self.line(),
                                    format!(
                                        "Inside a Chart expected `Series <field>`, `XAxis min..=max`, \
                                         `YAxis min..=max`, `Scatter`, or `End Chart`, found {:?}.",
                                        other
                                    ),
                                );
                                return None;
                            }
                        }
                    }
                } else {
                    fields.push(self.expect_ident("for the Chart's Vec<Struct> field")?);
                    while self.eat(&Tok::Comma) {
                        fields.push(self.expect_ident("for the next series field")?);
                    }
                    match self.peek().clone() {
                        Tok::Ident(w) if w.eq_ignore_ascii_case("Scatter") => {
                            self.advance();
                            scatter = true;
                        }
                        Tok::Ident(w) if w.eq_ignore_ascii_case("Line") => {
                            self.advance();
                        }
                        _ => {}
                    }
                    self.eat(&Tok::Newline);
                }
                Some(ViewNode::Chart { fields, scatter, x_bounds, y_bounds })
            }
            "input" => {
                // `Input field` + optional `On Submit <Event>` — a text entry line.
                self.advance();
                let field = self.expect_ident("for the input's bound String field")?;
                self.eat(&Tok::Newline);
                let mut on_submit = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Submit")?;
                            on_submit = Some(self.expect_ident("for the submit event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Input")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside an Input expected `On Submit <event>` or `End Input`, \
                                     found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::Input { field, on_submit })
            }
            "memo" => {
                // `Memo field` … `End Memo` — a multi-line String editor (Screen).
                self.advance();
                let field = self.expect_ident("for the Memo's bound String field")?;
                self.eat(&Tok::Newline);
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Memo")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Memo expected `End Memo`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::Memo { field })
            }
            "list" => {
                // `List field` + optional `On Select <Event>` — a selectable list.
                self.advance();
                let field = self.expect_ident("for the list's items field")?;
                self.eat(&Tok::Newline);
                let mut on_select = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            // `Select` lexes to a keyword token (Select-Case migration).
                            self.expect(&Tok::Select, "in `On Select`")?;
                            on_select = Some(self.expect_ident("for the select event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("List")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a List expected `On Select <event>` or `End List`, \
                                     found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::List { field, on_select })
            }
            "table" => {
                // `Table field` + optional `On Select <Event>` — a row-selectable table.
                self.advance();
                let field = self.expect_ident("for the table's rows field")?;
                self.eat(&Tok::Newline);
                let mut on_select = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            self.expect(&Tok::Select, "in `On Select`")?;
                            on_select = Some(self.expect_ident("for the select event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Table")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Table expected `On Select <event>` or `End Table`, \
                                     found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::Table { field, on_select })
            }
            "canvas" => {
                // `Canvas Board [Width 300] [Height 200]` — a drawing surface.
                self.advance();
                let name = self.expect_ident("for the canvas name")?;
                let mut width = None;
                let mut height = None;
                loop {
                    match self.peek().clone() {
                        Tok::Ident(w) if w.eq_ignore_ascii_case("Width") => {
                            self.advance();
                            width = Some(self.parse_array_size()? as u16);
                        }
                        Tok::Ident(w) if w.eq_ignore_ascii_case("Height") => {
                            self.advance();
                            height = Some(self.parse_array_size()? as u16);
                        }
                        _ => break,
                    }
                }
                self.eat(&Tok::Newline);
                Some(ViewNode::Canvas { name, width, height })
            }
            "button" => {
                self.advance();
                let label = self.parse_expr()?;
                self.eat(&Tok::Newline);
                let mut on_click = None;
                let mut enabled = None;
                loop {
                    self.skip_newlines();
                    match self.peek().clone() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Click")?;
                            on_click = Some(self.expect_ident("for the click event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::Ident(w) if w.eq_ignore_ascii_case("Enabled") => {
                            self.advance();
                            enabled = Some(self.parse_expr()?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Button")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Button expected `On Click <event>`, `Enabled <expr>`, \
                                     or `End Button`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::Button { label, on_click, enabled })
            }
            "textinput" => {
                self.advance();
                let placeholder = self.parse_expr()?;
                // The bound state field follows the placeholder: `TextInput "p", name`.
                self.expect(&Tok::Comma, "after the placeholder — `TextInput \"hint\", field`")?;
                let value = self.expect_ident("for the bound state field")?;
                self.eat(&Tok::Newline);
                let mut on_input = None;
                let mut on_submit = None;
                let mut secure = false;
                loop {
                    self.skip_newlines();
                    match self.peek().clone() {
                        Tok::On => {
                            self.advance();
                            let kind = self.expect_ident("after `On` — `Input` or `Submit`")?;
                            match kind.to_ascii_lowercase().as_str() {
                                "input" => {
                                    on_input = Some(self.expect_ident("for the input event")?);
                                }
                                "submit" => {
                                    on_submit = Some(self.expect_ident("for the submit event")?);
                                }
                                _ => {
                                    self.diags.error_at(
                                        self.span(),
                                        self.line(),
                                        format!(
                                            "Inside a TextInput expected `On Input` or `On Submit`, \
                                             found `On {kind}`."
                                        ),
                                    );
                                    return None;
                                }
                            }
                            self.eat(&Tok::Newline);
                        }
                        Tok::Ident(w) if w.eq_ignore_ascii_case("Secure") => {
                            self.advance();
                            secure = true;
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("TextInput")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a TextInput expected `On Input <event>`, \
                                     `On Submit <event>`, `Secure`, or `End TextInput`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::TextInput {
                    placeholder,
                    value,
                    on_input,
                    on_submit,
                    secure,
                })
            }
            "textarea" => {
                // Multi-line editor: just the bound field — the edit handler is
                // generated, so there's no `On …` clause.
                self.advance();
                let value = self.expect_ident("for the bound TextArea field")?;
                self.eat(&Tok::Newline);
                Some(ViewNode::TextArea { value })
            }
            "checkbox" => {
                self.advance();
                let label = self.parse_expr()?;
                // The bound bool field follows the label: `Checkbox "Agree", ok`.
                self.expect(&Tok::Comma, "after the label — `Checkbox \"label\", field`")?;
                let value = self.expect_ident("for the bound state field")?;
                self.eat(&Tok::Newline);
                let mut on_toggle = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Toggle")?;
                            on_toggle = Some(self.expect_ident("for the toggle event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Checkbox")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Checkbox expected `On Toggle <event>` or \
                                     `End Checkbox`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::Checkbox { label, value, on_toggle })
            }
            "slider" => {
                self.advance();
                // Range first, then the bound field: `Slider 0..=100, volume`.
                let min = self.parse_expr()?;
                self.expect(&Tok::DotDotEq, "for the slider range — `min..=max`")?;
                let max = self.parse_expr()?;
                self.expect(&Tok::Comma, "after the range — `Slider min..=max, field`")?;
                let value = self.expect_ident("for the bound state field")?;
                self.eat(&Tok::Newline);
                let mut on_change = None;
                let mut step = None;
                loop {
                    self.skip_newlines();
                    match self.peek().clone() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Change")?;
                            on_change = Some(self.expect_ident("for the change event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::Step => {
                            self.advance();
                            step = Some(self.parse_expr()?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Slider")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Slider expected `On Change <event>`, `Step n`, or \
                                     `End Slider`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                // Iced sliders always report changes, so the event is required.
                let on_change = match on_change {
                    Some(ev) => ev,
                    None => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            "A Slider needs `On Change <event>` — Iced sliders always report \
                             movement, so there must be an event to receive the new value.",
                        );
                        return None;
                    }
                };
                Some(ViewNode::Slider { min, max, value, on_change, step })
            }
            "toggler" => {
                self.advance();
                let label = self.parse_expr()?;
                self.expect(&Tok::Comma, "after the label — `Toggler \"label\", field`")?;
                let value = self.expect_ident("for the bound state field")?;
                self.eat(&Tok::Newline);
                let mut on_toggle = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Toggle")?;
                            on_toggle = Some(self.expect_ident("for the toggle event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Toggler")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Toggler expected `On Toggle <event>` or \
                                     `End Toggler`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::Toggler { label, value, on_toggle })
            }
            "radio" => {
                self.advance();
                let label = self.parse_expr()?;
                self.expect(&Tok::Comma, "after the label — `Radio \"label\", field, OptionValue`")?;
                let value = self.expect_ident("for the bound state field")?;
                self.expect(&Tok::Comma, "before this button's value")?;
                let option = self.parse_expr()?;
                self.eat(&Tok::Newline);
                let mut on_select = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            // `Select` lexes to a keyword token (kept for the Select-Case
                            // migration error), so match the token, not an ident.
                            self.expect(&Tok::Select, "in `On Select`")?;
                            on_select = Some(self.expect_ident("for the select event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Radio")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error_at(
                                self.span(),
                                self.line(),
                                format!(
                                    "Inside a Radio expected `On Select <event>` or `End Radio`, \
                                     found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                let on_select = match on_select {
                    Some(ev) => ev,
                    None => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            "A Radio needs `On Select <event>` — selecting it must report which \
                             option was chosen.",
                        );
                        return None;
                    }
                };
                Some(ViewNode::Radio { label, value, option, on_select })
            }
            "progressbar" => {
                // Display-only: a range and the bound field, on one line (no events).
                self.advance();
                let min = self.parse_expr()?;
                self.expect(&Tok::DotDotEq, "for the progress range — `min..=max`")?;
                let max = self.parse_expr()?;
                self.expect(&Tok::Comma, "after the range — `ProgressBar min..=max, field`")?;
                let value = self.expect_ident("for the bound state field")?;
                self.eat(&Tok::Newline);
                Some(ViewNode::ProgressBar { min, max, value })
            }
            other => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!(
                        "Unknown widget `{}` (have: Column, Row, Frame, Tabs, Space, Scrollable, Rule, \
                         Text, Button, TextInput, Checkbox, Slider, Toggler, ProgressBar, Radio, \
                         TextArea, Chooser, Markdown, Image, Svg, Canvas, Input, Memo, List, Table, \
                         Gauge, Sparkline, BarChart, Chart, Match, If).",
                        other
                    ),
                );
                None
            }
        }
    }

    /// `Match <expr>` inside a view: each arm is `pattern => <widget>` (inline) or
    /// an indented block of widgets, just like the statement form (§Match) — but
    /// the bodies are view nodes, and each arm yields one Iced `Element`.
    fn parse_view_match(&mut self) -> Option<ViewNode> {
        self.expect(&Tok::Match, "")?;
        let scrutinee = self.parse_expr()?;
        self.expect(&Tok::Newline, "after the `Match` expression")?;

        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End | Tok::Eof) {
                break;
            }
            let pattern = self.parse_pattern()?;
            let guard = if self.eat(&Tok::If) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&Tok::FatArrow, "after the pattern — every arm is `pattern => widget`")?;
            let body = if matches!(self.peek(), Tok::Newline | Tok::Eof) {
                self.parse_view_arm_body()?
            } else {
                vec![self.parse_view_node()?]
            };
            arms.push(ViewArm { pattern, guard, body });
        }
        self.expect(&Tok::End, "to close the `Match`")?;
        self.expect(&Tok::Match, "after `End`")?;
        Some(ViewNode::Match { scrutinee, arms })
    }

    /// A view-match arm's indented body: widgets until the next arm (a line with
    /// `=>`) or `End Match`.
    fn parse_view_arm_body(&mut self) -> Option<Vec<ViewNode>> {
        let mut nodes = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End | Tok::Eof) || self.line_has_fat_arrow() {
                break;
            }
            nodes.push(self.parse_view_node()?);
        }
        Some(nodes)
    }

    /// `If <cond> Then … [ElseIf <cond> Then …] [Else …] End If` inside a view —
    /// each branch is a block of widgets.
    fn parse_view_if(&mut self) -> Option<ViewNode> {
        self.expect(&Tok::If, "")?;
        let cond = self.parse_expr()?;
        self.expect(&Tok::Then, "after the `If` condition")?;
        self.eat(&Tok::Newline);
        let mut branches = vec![(cond, self.parse_view_branch_body()?)];
        let mut else_body = None;
        loop {
            match self.peek() {
                Tok::ElseIf => {
                    self.advance();
                    let c = self.parse_expr()?;
                    self.expect(&Tok::Then, "after the `ElseIf` condition")?;
                    self.eat(&Tok::Newline);
                    branches.push((c, self.parse_view_branch_body()?));
                }
                Tok::Else => {
                    self.advance();
                    self.eat(&Tok::Newline);
                    else_body = Some(self.parse_view_branch_body()?);
                    break;
                }
                Tok::End => break,
                other => {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        format!("Inside a view `If` expected `ElseIf`, `Else`, or `End If`, found {:?}.", other),
                    );
                    return None;
                }
            }
        }
        self.expect(&Tok::End, "to close the view `If`")?;
        self.expect(&Tok::If, "after `End`")?;
        self.eat(&Tok::Newline);
        Some(ViewNode::If { branches, else_body })
    }

    /// A view-`If` branch body: widgets until `ElseIf` / `Else` / `End`.
    fn parse_view_branch_body(&mut self) -> Option<Vec<ViewNode>> {
        let mut nodes = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::ElseIf | Tok::Else | Tok::End | Tok::Eof) {
                break;
            }
            nodes.push(self.parse_view_node()?);
        }
        Some(nodes)
    }

    /// A container body: optional `Spacing N` / `Padding N` property lines mixed
    /// with the child widgets, up to `End <container>`.
    fn parse_container_body(
        &mut self,
        container: &str,
    ) -> Option<(Vec<ViewNode>, Option<u16>, Option<u16>)> {
        let mut children = Vec::new();
        let mut spacing = None;
        let mut padding = None;
        // A size line (`Length 3` / `Fill` / `Percent 40` / `Min 5`) applies to the
        // next child (a TUI layout constraint).
        let mut pending: Option<SizeConstraint> = None;
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End) {
                self.advance();
                self.expect_kw_ident(container)?;
                self.eat(&Tok::Newline);
                break;
            }
            // Container/child properties are lines, not widgets.
            if let Tok::Ident(w) = self.peek().clone() {
                let prop = w.to_ascii_lowercase();
                match prop.as_str() {
                    "spacing" | "padding" => {
                        self.advance();
                        let n = self.parse_array_size()? as u16;
                        self.eat(&Tok::Newline);
                        if prop == "spacing" {
                            spacing = Some(n);
                        } else {
                            padding = Some(n);
                        }
                        continue;
                    }
                    "length" | "percent" | "min" => {
                        self.advance();
                        let n = self.parse_array_size()? as u16;
                        self.eat(&Tok::Newline);
                        pending = Some(match prop.as_str() {
                            "length" => SizeConstraint::Length(n),
                            "percent" => SizeConstraint::Percent(n),
                            _ => SizeConstraint::Min(n),
                        });
                        continue;
                    }
                    "fill" => {
                        // `Fill` (=1) or `Fill N` (weighted).
                        self.advance();
                        let n = if matches!(self.peek(), Tok::Int(_)) {
                            self.parse_array_size()? as u16
                        } else {
                            1
                        };
                        self.eat(&Tok::Newline);
                        pending = Some(SizeConstraint::Fill(n));
                        continue;
                    }
                    _ => {}
                }
            }
            let child = self.parse_view_node()?;
            children.push(match pending.take() {
                Some(size) => ViewNode::Constrained { size, child: Box::new(child) },
                None => child,
            });
        }
        Some((children, spacing, padding))
    }

    fn parse_param(&mut self) -> Option<Param> {
        let line = self.line();
        let explicit_mode = if self.eat(&Tok::ByVal) {
            Some(ParamMode::ByVal)
        } else if self.eat(&Tok::ByRef) {
            Some(ParamMode::ByRef)
        } else {
            None
        };
        let name = self.expect_ident("for the parameter")?;
        self.expect(&Tok::As, "after the parameter name")?;
        let ty = self.parse_decl_type()?;

        // Fixed-size primitives default to ByVal; unknown-size types (String,
        // struct, collection) must be explicit about lending vs sharing.
        let fixed_size = matches!(&ty, DeclType::Plain(t) if t.is_fixed_size())
            || matches!(&ty, DeclType::Tuple(_));
        let mode = match explicit_mode {
            Some(m) => m,
            None if fixed_size => ParamMode::ByVal,
            // A String parameter defaults to ByVal — a read-only `&str` borrow.
            // Trying to change it is caught with a friendly error in the resolver.
            None if matches!(&ty, DeclType::Plain(Type::Text)) => ParamMode::ByVal,
            // Struct / collection parameters still require an explicit mode.
            None => {
                self.diags.error(
                    line,
                    format!(
                        "Parameter '{}' has an unknown size — say how it is passed: \
                         `ByVal {}` borrows it (read only), `ByRef {}` borrows it mutably.",
                        name, name, name
                    ),
                );
                ParamMode::ByVal
            }
        };
        Some(Param { name, ty, mode })
    }

    /// Parse an event's payload parameter list (the opening `(` is already eaten).
    /// Event params are message data, always taken **by value** — so they don't
    /// need an explicit `ByVal`/`ByRef` even for a `String` or enum.
    fn parse_params_until_rparen(&mut self) -> Option<Vec<Param>> {
        let mut params = Vec::new();
        if !matches!(self.peek(), Tok::RParen) {
            loop {
                let name = self.expect_ident("for the parameter")?;
                self.expect(&Tok::As, "after the parameter name")?;
                let ty = self.parse_decl_type()?;
                params.push(Param { name, ty, mode: ParamMode::ByVal });
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.expect(&Tok::RParen, "to close the parameter list")?;
        Some(params)
    }

    /// Emit the "use DateTime" redirect for a `Date` used in type position.
    fn reject_date(&mut self, line: usize) {
        self.diags.error(
            line,
            "Date isn't a built-in Bust type — a bare date with no calendar semantics is \
             just a number in disguise. Use `DateTime` from the standard library: \
             `Dim now As DateTime = DateTime.Now()`, then `.Add_Days(n)`, `.Format(...)`, etc.",
        );
    }

    fn parse_type(&mut self) -> Option<Type> {
        let line = self.line();
        let ty = match self.peek() {
            Tok::TyInteger => Type::Integer,
            Tok::TyLong => Type::Long,
            Tok::TyLongLong => Type::LongLong,
            Tok::TySingle => Type::Single,
            Tok::TyDouble => Type::Double,
            Tok::TyBoolean => Type::Boolean,
            Tok::TyByte => Type::Byte,
            // `Date` is no longer a built-in type (so `date` is free as an
            // identifier); redirect a type-position `Date` to the stdlib.
            Tok::Ident(w) if w.eq_ignore_ascii_case("Date") => {
                self.reject_date(line);
                return None;
            }
            Tok::TyString => Type::Text,
            Tok::TyCurrency => {
                self.diags.error(
                    line,
                    "Currency is not supported — Rust has no built-in fixed-point money type. \
                     Use Double (f64) for approximate amounts, or store integer minor units \
                     (cents) in a Long / LongLong.",
                );
                return None;
            }
            Tok::TyVariant => {
                self.diags.error(
                    line,
                    "Variant is not supported — Rust must know each value's type at compile \
                     time. Declare the concrete type you actually mean.",
                );
                return None;
            }
            other => {
                self.diags.error(
                    line,
                    format!(
                        "Expected a type (Integer, Long, LongLong, Single, Double, Boolean, \
                         Byte, String), found {:?}.",
                        other
                    ),
                );
                return None;
            }
        };
        self.advance();
        Some(ty)
    }

    /// Parse statements until a block-terminating keyword (handled by caller).
    /// Each statement ends at a line boundary; a trailing inline comment is
    /// kept as its own `//` line in the output.
    fn parse_block(&mut self) -> Option<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_block_end() {
                break;
            }
            // Record the statement's source line (comments don't need one).
            if !matches!(self.peek(), Tok::Comment(_)) {
                stmts.push(Stmt::LineMark(self.line()));
            }
            let Some(s) = self.parse_stmt() else {
                // The error is already recorded — resync at end of line and
                // keep going, so a half-typed statement costs one diagnostic,
                // not everything below it.
                self.recover_to_eol();
                continue;
            };
            stmts.push(s);
            // A multi-variable `Dim` leaves its extra declarations here.
            stmts.append(&mut self.dim_overflow);

            if let Tok::Comment(text) = self.peek().clone() {
                self.advance();
                stmts.push(Stmt::Comment(text));
            }

            if !matches!(self.peek(), Tok::Newline | Tok::Eof) && !self.at_block_end() {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!("Expected end of line after statement, found {:?}.", self.peek()),
                );
                self.recover_to_eol();
            }
        }
        Some(stmts)
    }

    fn at_block_end(&self) -> bool {
        matches!(
            self.peek(),
            Tok::End | Tok::ElseIf | Tok::Else | Tok::Case | Tok::Next | Tok::Loop | Tok::Eof
        )
    }

    /// Is the cursor at the first token of its line? Recovery uses this so a
    /// variable named `test` mid-expression isn't mistaken for a `Test` block.
    fn at_line_start(&self) -> bool {
        self.pos == 0 || matches!(self.toks[self.pos - 1].tok, Tok::Newline)
    }

    /// After a statement fails to parse, skip to the end of its line so the
    /// *next* statement is still analysed. The editor recompiles on every
    /// keystroke — mid-word — so one half-typed line must cost one error, not
    /// every diagnostic below it.
    fn recover_to_eol(&mut self) {
        while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
            self.advance();
        }
    }

    /// After a top-level item fails to parse, skip forward to the next thing
    /// that can start one — so a broken function doesn't silence the whole
    /// rest of the file. A closing `End Function`/`End Sub`/`End Type`/… on
    /// the way belongs to the broken item and is consumed with it.
    fn recover_to_item(&mut self) {
        let surface_kw = |w: &str| {
            matches!(
                w.to_ascii_lowercase().as_str(),
                "window" | "screen" | "page" | "canvas" | "sketch" | "test"
            )
        };
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Function
                | Tok::Sub
                | Tok::Type
                | Tok::Enum
                | Tok::Const
                | Tok::Public
                | Tok::Private
                | Tok::Use(_)
                    if self.at_line_start() =>
                {
                    return;
                }
                Tok::Ident(w) if self.at_line_start() && surface_kw(w) => return,
                Tok::End
                    if matches!(
                        self.peek2(),
                        Tok::Function | Tok::Sub | Tok::Type | Tok::Enum
                    ) || matches!(self.peek2(), Tok::Ident(w) if surface_kw(w)) =>
                {
                    self.advance();
                    self.advance();
                    return;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Parse a single statement. Line termination is handled by `parse_block`.
    fn parse_stmt(&mut self) -> Option<Stmt> {
        match self.peek().clone() {
            Tok::Comment(text) => {
                self.advance();
                Some(Stmt::Comment(text))
            }
            Tok::Dim => self.parse_dim(),
            Tok::Set => self.parse_set(),
            Tok::Return => {
                self.advance();
                // `Return` may stand alone (no value) or carry an expression.
                if matches!(self.peek(), Tok::Newline | Tok::Eof | Tok::Comment(_))
                    || self.at_block_end()
                {
                    Some(Stmt::Return(None))
                } else {
                    Some(Stmt::Return(Some(self.parse_expr()?)))
                }
            }
            // VB idiom: `Function = value` assigns the return value.
            Tok::Function => {
                self.advance();
                self.expect(&Tok::Eq, "in `Function = value`")?;
                Some(Stmt::Return(Some(self.parse_expr()?)))
            }
            Tok::If => self.parse_if(),
            Tok::Match => self.parse_match(),
            Tok::Select => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "`Select Case` has been replaced by `Match` … `End Match`, which maps \
                     straight to Rust's `match`. Each arm is `pattern => body` (no `Case`); \
                     patterns are real Rust — `Ok(n)`, `Some(x)`, `1 | 2`, `1..=10`, `_`.",
                );
                None
            }
            Tok::For => self.parse_for(),
            // A standalone inline Rust block (side effects; no value used).
            Tok::InlineRust(_) => Some(Stmt::Expr(self.parse_primary()?)),
            Tok::Const => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "Declare constants at the top of the file (module level), not inside a function.",
                );
                None
            }
            Tok::ReDim => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "ReDim isn't supported. Use a Vec (`Dim x As Vec<T>`), which grows on \
                     demand with `.push(...)` — no resizing dance needed.",
                );
                while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                    self.advance();
                }
                None
            }
            Tok::With => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "`With` blocks aren't supported — write the variable name out each time \
                     (e.g. `p.x = 1` / `p.y = 2`, not `With p` … `.x = 1`). It's a little more \
                     typing but far clearer about what you're touching.",
                );
                // Swallow the whole block (through `End With`) so its body
                // doesn't produce follow-on noise — this one error says it all.
                loop {
                    match self.peek() {
                        Tok::Eof => break,
                        Tok::End if matches!(self.peek2(), Tok::With) => {
                            self.advance();
                            self.advance();
                            break;
                        }
                        _ => {
                            self.advance();
                        }
                    }
                }
                None
            }
            Tok::Do => self.parse_do(),
            Tok::Continue => {
                self.advance();
                Some(Stmt::Continue)
            }
            Tok::Exit => {
                self.advance();
                match self.peek() {
                    Tok::Do | Tok::For => {
                        self.advance();
                        Some(Stmt::Break)
                    }
                    Tok::Function => {
                        self.advance();
                        Some(Stmt::Return(None))
                    }
                    other => {
                        self.diags.error_at(
                            self.span(),
                            self.line(),
                            format!("`Exit {:?}` is not supported — use `Exit Do`, `Exit For`, or `Exit Function`.", other),
                        );
                        None
                    }
                }
            }
            Tok::On => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "`On Error` is not supported. Errors propagate automatically; intercept a \
                     call with `Handle err` … `End Handle`, or fail with `RaiseError \"…\"`.",
                );
                // Swallow the rest of the line so we don't cascade more errors.
                while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                    self.advance();
                }
                None
            }
            // `Assert <expr>` inside a `Test` block. `Assert` isn't a reserved
            // word globally (only recognised at statement start), so a variable
            // elsewhere is unaffected; what follows is a full expression, so
            // `Assert x = 3` reads the `=` as equality (→ `assert_eq!`).
            Tok::Ident(w) if w.eq_ignore_ascii_case("assert") => {
                self.advance();
                Some(Stmt::Assert(self.parse_expr()?))
            }
            // `Log.<Level> <expr>` — a leveled log (like `Debug.Print`): `Log.Warn
            // "…"`, `.Error`, `.Debug`, `.Info`. Recognised only when a known level
            // follows the dot, so `log.Push(x)` on a variable is untouched.
            Tok::Ident(w)
                if w.eq_ignore_ascii_case("log")
                    && matches!(self.peek2(), Tok::Dot)
                    && log_level(self.peek3()).is_some() =>
            {
                self.advance(); // `Log`
                self.advance(); // `.`
                let level = log_level(self.peek()).unwrap();
                self.advance(); // the level word
                Some(Stmt::Log(level, self.parse_expr()?))
            }
            // `Log <expr>` — the bare logging verb (Info), like `Debug.Print` but
            // to a file. Only at statement start, and only when what follows begins
            // an expression *argument*: `Log "msg"`, `Log count`. A following
            // `(`/`.`/`[`/`=` means `log` is being used as a value instead —
            // `Log(x)` is the natural-log builtin, `log = 5` / `log.Push(x)` a
            // variable — so those fall through untouched.
            Tok::Ident(w)
                if w.eq_ignore_ascii_case("log")
                    && !matches!(
                        self.peek2(),
                        Tok::LParen | Tok::Dot | Tok::LBracket | Tok::Eq
                    ) =>
            {
                self.advance();
                Some(Stmt::Log(LogLevel::Info, self.parse_expr()?))
            }
            // `Connect <source>.<Signal> To <Handler>` — wire a signal to a
            // handler method (only in a node body). `source` is `Me` (own signal)
            // or a `GetNode` handle. Carried as a call on a reserved receiver so
            // no new `Stmt` variant leaks into the other backends (like `Emit`).
            Tok::Ident(w) if w.eq_ignore_ascii_case("connect") && matches!(self.peek2(), Tok::Ident(_)) => {
                let span = self.span();
                self.advance(); // `Connect`
                let source = self.expect_ident("for the signal's source (`Me` or a node)")?;
                self.expect(&Tok::Dot, "before the signal name")?;
                let signal = self.expect_ident("for the signal name")?;
                self.expect(&Tok::To, "before the handler (`… To OnPinged`)")?;
                let handler = self.expect_ident("for the handler name after `To`")?;
                self.eat(&Tok::Newline);
                let recv = ExprKind::Ident(source).at(span);
                let args = vec![ExprKind::Str(signal).at(span), ExprKind::Str(handler).at(span)];
                Some(Stmt::Expr(
                    ExprKind::MethodCall {
                        recv: Box::new(recv),
                        method: "__vbr_connect".to_string(),
                        args,
                    }
                    .at(span),
                ))
            }
            // `Emit <Signal>[(args)]` — fire a Godot signal (only in a node body).
            // Like `Assert`, `Emit` is recognised only at statement start followed
            // by a name, so a variable/function `emit` elsewhere is untouched. It
            // lowers (Godot backend) to `self.signals().<signal>().emit(args)`; it
            // is carried as a call on a reserved receiver so no new `Stmt` variant
            // leaks into the other backends.
            Tok::Ident(w) if w.eq_ignore_ascii_case("emit") && matches!(self.peek2(), Tok::Ident(_)) => {
                let span = self.span();
                self.advance(); // `Emit`
                let signal = self.expect_ident("for the signal name after `Emit`")?;
                let args = if self.eat(&Tok::LParen) {
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Tok::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.eat(&Tok::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&Tok::RParen, "to close the signal arguments")?;
                    args
                } else {
                    Vec::new()
                };
                let recv = ExprKind::Ident("__vbr_emit".to_string()).at(span);
                Some(Stmt::Expr(
                    ExprKind::MethodCall { recv: Box::new(recv), method: signal, args }.at(span),
                ))
            }
            Tok::Ident(name) => self.parse_ident_stmt(name),
            other => {
                self.diags
                    .error_at(self.span(), self.line(), format!("Unexpected {:?} at start of statement.", other));
                None
            }
        }
    }

    fn parse_dim(&mut self) -> Option<Stmt> {
        let line = self.line();
        self.expect(&Tok::Dim, "")?;

        // `Dim (a, b) As (T, U) = expr` — a typed destructure. Chiefly for a
        // `Python` block that returns several values, extracted in one GIL scope.
        if self.eat(&Tok::LParen) {
            let mut names = vec![self.expect_ident("in a destructuring `Dim (a, b)`")?];
            while self.eat(&Tok::Comma) {
                names.push(self.expect_ident("in a destructuring `Dim (a, b)`")?);
            }
            self.expect(&Tok::RParen, "to close the destructured names")?;
            let ty = if self.eat(&Tok::As) {
                Some(self.parse_decl_type()?)
            } else {
                None
            };
            self.expect(&Tok::Eq, "in a destructuring `Dim (a, b) = …`")?;
            let value = self.parse_expr()?;
            if matches!(value.kind, ExprKind::InlinePython { .. }) && ty.is_none() {
                self.diags.error(
                    line,
                    "A `Python` block that returns several values needs their types: \
                     `Dim (name, data) As (String, Vec<Double>) = Python … End Python`. \
                     The Rust tuple they're extracted into must be known.",
                );
                return None;
            }
            return Some(Stmt::DestructureDim { names, ty, value });
        }

        let name_span = self.span();
        let name = self.expect_ident("after `Dim`")?;

        // `Dim a, b = expr` destructures a tuple (untyped, names inferred).
        if matches!(self.peek(), Tok::Comma) {
            let mut names = vec![name];
            while self.eat(&Tok::Comma) {
                names.push(self.expect_ident("for the destructured name")?);
            }
            // `Dim a, b As Integer` is VBA's old habit, where `a` is silently a
            // Variant. Bust has no Variant, so a shared trailing `As` is a trap
            // rather than a shorthand — steer to a type on each variable.
            if matches!(self.peek(), Tok::As) {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "In Bust every variable needs its own type: \
                     `Dim a As Long, b As Integer`. VBA's `Dim a, b As Integer` \
                     would leave `a` an untyped Variant, which Bust doesn't have.",
                );
                return None;
            }
            self.expect(&Tok::Eq, "in a tuple destructuring (`Dim a, b = …`)")?;
            let value = self.parse_expr()?;
            return Some(Stmt::DestructureDim { names, ty: None, value });
        }

        // `Dim name = Rust … End Rust` — an opaque handle. The only `As`-less
        // single `Dim`: the type is whatever Rust infers, hidden from Bust.
        if self.eat(&Tok::Eq) {
            if let Tok::InlineRust(raw) = self.peek().clone() {
                self.advance();
                return Some(Stmt::HandleDim { name, raw, line });
            }
            // `Dim h = Python … End Python` (no `As`) — an opaque `PyObject` handle,
            // the Python counterpart of the inline-Rust handle above. Holds a value
            // Bust has no type for; pass it back into a later `Python(h)` block.
            if let Tok::InlinePython { args, body } = self.peek().clone() {
                let span = self.span();
                self.advance();
                return Some(Stmt::Dim {
                    name,
                    name_span,
                    ty: DeclType::Named("PyObject".to_string()),
                    init: Some(
                        ExprKind::InlinePython {
                            inputs: split_py_args(&args),
                            body,
                        }
                        .at(span),
                    ),
                    deferred: false,
                    line,
                });
            }
            self.diags.error(
                line,
                "A `Dim` needs a type: `Dim x As Long`. The one exception is \
                 `Dim h = Rust … End Rust`, which makes an opaque Rust handle whose \
                 type Rust infers — Bust can pass it back into another `Rust` block but \
                 can't use it as a value.",
            );
            return None;
        }

        // The common case: a typed declaration — and possibly several,
        // comma-separated (`Dim a As Long, b As Integer`). Each variable carries
        // its own `As Type`; the first clause is returned and any others are
        // queued for the surrounding block to pick up.
        let first = self.parse_dim_clause(name, name_span, line)?;
        while self.eat(&Tok::Comma) {
            let clause_span = self.span();
            let clause_name = self.expect_ident("after `,` in a multi-variable `Dim`")?;
            let clause = self.parse_dim_clause(clause_name, clause_span, line)?;
            self.dim_overflow.push(clause);
        }
        Some(first)
    }

    /// Parse one typed declaration once its name has been read: the optional
    /// parenthesised dimension spec, the required `As Type`, and any
    /// initialiser. A single `Dim` line may hold several of these in a row,
    /// separated by commas.
    fn parse_dim_clause(&mut self, name: String, name_span: Span, line: usize) -> Option<Stmt> {
        // An optional dimension spec in parens: `()` `(,)` `(N)` `(R, C)`.
        let dim = self.parse_dim_spec()?;

        self.expect(&Tok::As, "after the variable name")?;
        let ty = match dim {
            DimSpec::None => self.parse_decl_type()?,
            DimSpec::Empty1D => DeclType::Vec(Box::new(self.parse_decl_type()?)),
            DimSpec::Empty2D => {
                DeclType::Vec(Box::new(DeclType::Vec(Box::new(self.parse_decl_type()?))))
            }
            DimSpec::Fixed1D(n) => DeclType::Array(self.parse_type()?, n),
            DimSpec::Fixed2D(r, c) => DeclType::Array2D(self.parse_type()?, r, c),
        };

        // Plain scalars may carry an initialiser; a struct must be fully built at
        // creation; fixed arrays auto-zero; collections start empty.
        let init = match &ty {
            DeclType::Plain(_) | DeclType::Tuple(_) => {
                if self.eat(&Tok::Eq) {
                    Some(self.parse_expr()?)
                } else {
                    None
                }
            }
            DeclType::Named(_) => {
                if self.eat(&Tok::Eq) {
                    Some(self.parse_expr()?)
                } else {
                    self.diags.error(
                        line,
                        "A struct must be fully initialised at creation — \
                         `Dim p As Person = Person { name: \"...\", age: ... }`. \
                         You cannot declare it empty and fill fields in later.",
                    );
                    return None;
                }
            }
            // A collection may take an initialiser (e.g. an iterator `.collect()`).
            DeclType::Vec(_) | DeclType::Map(..) | DeclType::Result(..) | DeclType::Option(_) => {
                if self.eat(&Tok::Eq) {
                    Some(self.parse_expr()?)
                } else {
                    None
                }
            }
            // Fixed arrays are auto-zeroed.
            DeclType::Array(..) | DeclType::Array2D(..) => None,
        };
        // `Dim x As T = F() Handle err` — declare `x`, then intercept the call.
        if let Some(call) = init {
            if self.at_handle() {
                let target = ExprKind::Ident(name.clone()).at(name_span);
                if let Some(handle) = self.parse_handle(Some(target), call) {
                    self.dim_overflow.push(handle);
                }
                return Some(Stmt::Dim {
                    name,
                    name_span,
                    ty,
                    init: None,
                    deferred: true,
                    line,
                });
            }
            return Some(Stmt::Dim {
                name,
                name_span,
                ty,
                init: Some(call),
                deferred: false,
                line,
            });
        }
        Some(Stmt::Dim {
            name,
            name_span,
            ty,
            init: None,
            deferred: false,
            line,
        })
    }

    /// Parse the parenthesised dimension spec after a `Dim` name, if any.
    fn parse_dim_spec(&mut self) -> Option<DimSpec> {
        if !self.eat(&Tok::LParen) {
            return Some(DimSpec::None);
        }
        if self.eat(&Tok::RParen) {
            return Some(DimSpec::Empty1D);
        }
        if self.eat(&Tok::Comma) {
            // `(,)` — a dynamic 2D array.
            self.expect(&Tok::RParen, "to close `(,)`")?;
            return Some(DimSpec::Empty2D);
        }
        let n = self.parse_array_size()?;
        if self.eat(&Tok::Comma) {
            let c = self.parse_array_size()?;
            self.expect(&Tok::RParen, "to close the array dimensions")?;
            Some(DimSpec::Fixed2D(n, c))
        } else {
            self.expect(&Tok::RParen, "to close the array size")?;
            Some(DimSpec::Fixed1D(n))
        }
    }

    fn parse_array_size(&mut self) -> Option<usize> {
        if let Tok::Int(n) = self.peek() {
            let n = *n as usize;
            self.advance();
            Some(n)
        } else {
            self.diags.error_at(
                self.span(),
                self.line(),
                "An array size must be an integer literal, e.g. `Dim x(10) As Long`.",
            );
            None
        }
    }

    /// The one recursive type parser — used in every position (Dim, field,
    /// parameter, return). Handles `Result<T>`, `Option<T>`, `Vec<T>`,
    /// `HashMap<K, V>`, tuples, primitives, and named structs, nested freely.
    fn parse_decl_type(&mut self) -> Option<DeclType> {
        // `New` is a VB-ism with no meaning in Bust (Rust has no uninitialised
        // objects) — accept it out of habit, but nudge toward dropping it.
        if self.eat(&Tok::New) {
            self.diags.warn(
                self.line(),
                "`New` isn't needed in Bust — a value is created by its declaration. \
                 Write `Dim v As Vec<T>` / `As HashMap<K, V>` without `New`.",
            );
        }
        if matches!(self.peek(), Tok::LParen) {
            return Some(DeclType::Tuple(self.parse_tuple_types()?));
        }
        if let Tok::Ident(name) = self.peek().clone() {
            match name.as_str() {
                "Vec" => {
                    self.advance();
                    self.expect(&Tok::Lt, "before the element type, e.g. Vec<Long>")?;
                    let t = self.parse_decl_type()?;
                    self.expect(&Tok::Gt, "to close `Vec<...>`")?;
                    Some(DeclType::Vec(Box::new(t)))
                }
                "HashMap" => {
                    self.advance();
                    self.expect(&Tok::Lt, "before the key type, e.g. HashMap<String, Long>")?;
                    let k = self.parse_decl_type()?;
                    self.expect(&Tok::Comma, "between the key and value types")?;
                    let v = self.parse_decl_type()?;
                    self.expect(&Tok::Gt, "to close `HashMap<...>`")?;
                    Some(DeclType::Map(Box::new(k), Box::new(v)))
                }
                "Result" => {
                    self.advance();
                    self.expect(&Tok::Lt, "before the type, e.g. Result<Long>")?;
                    let t = self.parse_decl_type()?;
                    // `Result<T, E>` — full form; `Result<T>` — E defaults to String.
                    let e = if self.eat(&Tok::Comma) {
                        self.parse_decl_type()?
                    } else {
                        DeclType::Plain(Type::Text)
                    };
                    self.expect(&Tok::Gt, "to close `Result<...>`")?;
                    Some(DeclType::Result(Box::new(t), Box::new(e)))
                }
                "Option" => {
                    self.advance();
                    self.expect(&Tok::Lt, "before the type, e.g. Option<String>")?;
                    let t = self.parse_decl_type()?;
                    self.expect(&Tok::Gt, "to close `Option<...>`")?;
                    Some(DeclType::Option(Box::new(t)))
                }
                _ if name.eq_ignore_ascii_case("Date") => {
                    self.reject_date(self.line());
                    None
                }
                _ => {
                    self.advance();
                    Some(DeclType::Named(name))
                }
            }
        } else {
            Some(DeclType::Plain(self.parse_type()?))
        }
    }

    /// Parse a tuple type list `(Type, Type, …)`.
    fn parse_tuple_types(&mut self) -> Option<Vec<DeclType>> {
        self.expect(&Tok::LParen, "to start a tuple type")?;
        let mut types = Vec::new();
        if !matches!(self.peek(), Tok::RParen) {
            loop {
                types.push(self.parse_decl_type()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.expect(&Tok::RParen, "to close the tuple type")?;
        Some(types)
    }

    fn parse_set(&mut self) -> Option<Stmt> {
        let set_span = self.span();
        self.expect(&Tok::Set, "")?;
        // `Set Pixel x, y, color` — a drawing verb (the raster cousin of Fill).
        if let Tok::Ident(w) = self.peek().clone() {
            if w.eq_ignore_ascii_case("Pixel") && !matches!(self.peek2(), Tok::Eq) {
                self.advance(); // Pixel
                let x = self.parse_expr()?;
                self.expect(&Tok::Comma, "after x — `Set Pixel <x>, <y>, <color>`")?;
                let y = self.parse_expr()?;
                self.expect(&Tok::Comma, "after y — `Set Pixel <x>, <y>, <color>`")?;
                let color = self.parse_expr()?;
                return Some(Stmt::Draw(DrawCmd::Pixel { x, y, color }));
            }
        }
        let mutable = self.eat(&Tok::Mut);
        let name = self.expect_ident("after `Set`")?;
        self.expect(&Tok::Eq, "in a `Set` borrow")?;
        // `Set x = Nothing` is the VB6 object-release habit. In Bust `Set` binds a
        // borrow (a Rust reference), not an assignment — so steer to the plain
        // form `x = Nothing`, which is what actually releases the value.
        if matches!(self.peek(), Tok::Nothing) {
            self.diags.error_at(
                set_span,
                self.line(),
                format!(
                    "`Set` in Bust binds a borrow, not a VB6 object assignment. To release \
                     a value, drop the `Set` — write `{} = Nothing`.",
                    name
                ),
            );
            return None;
        }
        let value = self.parse_expr()?;
        Some(Stmt::Set {
            name,
            mutable,
            value,
        })
    }

    /// An identifier at statement start is either `Debug.Print expr` or `name = expr`.
    fn parse_ident_stmt(&mut self, name: String) -> Option<Stmt> {
        // `RaiseError "…"` / `RaiseError err` — fail from this function.
        if name.eq_ignore_ascii_case("raiseerror") {
            self.advance();
            return Some(Stmt::RaiseError(self.parse_expr()?));
        }

        if name.eq_ignore_ascii_case("into")
            && matches!(self.peek2(), Tok::Ident(_))
            && !matches!(
                self.peek3(),
                Tok::Eq | Tok::PlusEq | Tok::MinusEq | Tok::StarEq | Tok::SlashEq | Tok::Dot | Tok::LParen
            )
        {
            self.advance(); // Into
            let target = self.expect_ident("for the `Pixels` to draw into — `Into spr`")?;
            self.eat(&Tok::Newline);
            let body = self.parse_block()?;
            self.expect(&Tok::End, "to close `Into`")?;
            self.expect_kw_ident("Into")?;
            return Some(Stmt::GpuInto { name: target, body });
        }

        // Drawing verbs (only meaningful in a `Draw` block / paint function). We
        // treat them as draw commands when they lead an operand rather than an
        // assignment, so a variable named `Text`/`Fill`/`Stroke` still assigns.
        let is_draw_verb = matches!(
            name.to_ascii_lowercase().as_str(),
            "fill" | "stroke" | "text" | "pixel" | "copy" | "clear"
        ) && !matches!(
            self.peek2(),
            Tok::Eq | Tok::PlusEq | Tok::MinusEq | Tok::StarEq | Tok::SlashEq | Tok::Dot
                | Tok::LParen | Tok::Newline | Tok::Eof
        );
        if is_draw_verb {
            return self.parse_draw_cmd(&name);
        }

        if name.eq_ignore_ascii_case("Debug") {
            self.advance(); // Debug
            self.expect(&Tok::Dot, "after `Debug`")?;
            let method = self.expect_ident("after `Debug.`")?;
            if !method.eq_ignore_ascii_case("Print") {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!("`Debug.{}` is not supported yet — only `Debug.Print`.", method),
                );
                return None;
            }
            let value = self.parse_expr()?;
            return Some(Stmt::Print(value));
        }

        // `MsgBox msg` has no window in a terminal app, so it prints the message.
        if name.eq_ignore_ascii_case("MsgBox") {
            self.advance(); // MsgBox
            self.diags.note(
                "msgbox-cli",
                "MsgBox has no window in a terminal app, so Bust prints it to the terminal \
                 (like Debug.Print). InputBox reads a line of input back.",
            );
            let value = self.parse_expr()?;
            return Some(Stmt::Print(value));
        }

        // `Sleep 500` — VB6's kernel32 Sleep (milliseconds), paren-less like
        // the original Sub call. `Sleep(500)` parses the same way.
        if name.eq_ignore_ascii_case("Sleep") {
            self.advance(); // Sleep
            let ms = self.parse_expr()?;
            let span = ms.span;
            return Some(Stmt::Expr(
                ExprKind::Call { name: "Sleep".to_string(), args: vec![ms] }.at(span),
            ));
        }

        // Parse a place expression (Ident or `a.field`) or a call. `parse_primary`
        // stops before binary operators, so a top-level `=` isn't mistaken for the
        // equality operator.
        let target = self.parse_primary()?;
        if self.eat(&Tok::Eq) {
            // `x = Nothing` — release the value. Only a bare variable can be
            // destroyed (not a field/index); the RHS is the `Nothing` keyword.
            if matches!(self.peek(), Tok::Nothing) {
                self.advance();
                if let ExprKind::Ident(n) = target.kind {
                    return Some(Stmt::Destroy { name: n, name_span: target.span });
                }
                self.diags.error_at(
                    target.span,
                    self.line(),
                    "`= Nothing` releases a variable, so its left side must be a plain \
                     variable name (not a field or element).",
                );
                return None;
            }
            let value = self.parse_expr()?;
            if self.at_handle() {
                self.parse_handle(Some(target), value)
            } else {
                Some(Stmt::Assign { target, value, op: None })
            }
        } else if let Some(op) = self.compound_assign_op() {
            self.advance();
            let value = self.parse_expr()?;
            Some(Stmt::Assign { target, value, op: Some(op) })
        } else if self.at_handle() {
            self.parse_handle(None, target)
        } else {
            Some(Stmt::Expr(target))
        }
    }

    fn at_handle(&self) -> bool {
        matches!(self.peek(), Tok::Ident(w) if w.eq_ignore_ascii_case("handle"))
    }

    /// `… Handle err` newline body `End Handle`.
    fn parse_handle(&mut self, target: Option<Expr>, call: Expr) -> Option<Stmt> {
        let line = self.line();
        self.advance(); // `Handle`
        let err_name = self.expect_ident("for the error binding (`Handle err`)")?;
        self.expect(&Tok::Newline, "after `Handle err`")?;
        let body = self.parse_block()?;
        self.expect(&Tok::End, "to close the Handle")?;
        match self.peek() {
            Tok::Ident(w) if w.eq_ignore_ascii_case("handle") => {
                self.advance();
            }
            _ => {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    "Expected `Handle` after `End`.",
                );
                return None;
            }
        }
        Some(Stmt::HandleErr {
            target,
            call,
            err_name,
            body,
            line,
        })
    }

    /// `+=` / `-=` / `*=` / `/=` at the current position → its arithmetic op.
    fn compound_assign_op(&self) -> Option<BinOp> {
        match self.peek() {
            Tok::PlusEq => Some(BinOp::Add),
            Tok::MinusEq => Some(BinOp::Sub),
            Tok::StarEq => Some(BinOp::Mul),
            Tok::SlashEq => Some(BinOp::Div),
            _ => None,
        }
    }

    fn parse_if(&mut self) -> Option<Stmt> {
        let line = self.line();
        self.expect(&Tok::If, "")?;
        let cond = self.parse_expr()?;
        // `If <expr> Is <pattern> Then …` — VB-flavoured `if let`. `Is` is a VB
        // keyword (VB6's `Is Nothing`); here it means "matches the pattern". Lower
        // to a `Match` flagged `if_let` (so the Rust backend emits `if let` and the
        // others emit an ordinary `match`).
        if matches!(self.peek(), Tok::Ident(n) if n.eq_ignore_ascii_case("is")) {
            return self.parse_if_let(cond, line);
        }
        self.expect(&Tok::Then, "after the `If` condition")?;
        // Single-line form: `If cond Then <stmt> [Else <stmt>]` — a statement
        // follows `Then` on the same line, and there is no `End If`.
        if !matches!(self.peek(), Tok::Newline) {
            let then_stmt = self.parse_stmt()?;
            let mut then_body = vec![then_stmt];
            then_body.append(&mut self.dim_overflow);
            let else_body = if self.eat(&Tok::Else) {
                let else_stmt = self.parse_stmt()?;
                let mut eb = vec![else_stmt];
                eb.append(&mut self.dim_overflow);
                Some(eb)
            } else {
                None
            };
            return Some(Stmt::If {
                branches: vec![(cond, then_body)],
                else_body,
            });
        }
        self.expect(&Tok::Newline, "after `Then`")?;
        let body = self.parse_block()?;

        let mut branches = vec![(cond, body)];
        let mut else_body = None;

        loop {
            match self.peek() {
                Tok::ElseIf => {
                    self.advance();
                    let cond = self.parse_expr()?;
                    self.expect(&Tok::Then, "after the `ElseIf` condition")?;
                    self.expect(&Tok::Newline, "after `Then`")?;
                    let body = self.parse_block()?;
                    branches.push((cond, body));
                }
                Tok::Else => {
                    self.advance();
                    self.expect(&Tok::Newline, "after `Else`")?;
                    else_body = Some(self.parse_block()?);
                    break;
                }
                _ => break,
            }
        }

        self.expect(&Tok::End, "to close the `If`")?;
        self.expect(&Tok::If, "after `End`")?;
        Some(Stmt::If {
            branches,
            else_body,
        })
    }

    /// `If <expr> Is <pattern> Then …` — VB-flavoured `if let`. The `Is` has been
    /// peeked (not yet consumed). Lowers to a `Match` flagged `if_let`, with the
    /// pattern arm plus a trailing `_ => {}` (so non-Rust backends stay valid).
    fn parse_if_let(&mut self, scrutinee: Expr, line: usize) -> Option<Stmt> {
        self.advance(); // consume `Is`
        // Capture the pattern raw (like a Match arm), up to `Then`.
        let mut parts: Vec<String> = Vec::new();
        while !matches!(self.peek(), Tok::Then | Tok::Newline | Tok::Eof) {
            let t = self.advance();
            parts.push(pattern_tok_src(&t, self.line()));
        }
        if parts.is_empty() {
            self.diags.error_at(
                self.span(),
                self.line(),
                "Expected a pattern after `Is` — e.g. `If result Is Some(x) Then`.",
            );
            return None;
        }
        let pattern = parts.join(" ");
        self.expect(&Tok::Then, "after the `Is` pattern")?;

        // Single-line (`If x Is Some(v) Then <stmt> [Else <stmt>]`) or a block
        // ending in `End If`, with an optional `Else`. The else-body becomes the
        // `_ => …` arm — which is the Rust `else` block *and* the wildcard arm the
        // other backends render.
        let (body, else_body) = if !matches!(self.peek(), Tok::Newline) {
            let mut b = vec![self.parse_stmt()?];
            b.append(&mut self.dim_overflow);
            let eb = if self.eat(&Tok::Else) {
                let mut e = vec![self.parse_stmt()?];
                e.append(&mut self.dim_overflow);
                e
            } else {
                Vec::new()
            };
            (b, eb)
        } else {
            self.expect(&Tok::Newline, "after `Then`")?;
            let b = self.parse_block()?;
            let eb = if self.eat(&Tok::Else) {
                self.expect(&Tok::Newline, "after `Else`")?;
                self.parse_block()?
            } else {
                Vec::new()
            };
            self.expect(&Tok::End, "to close the `If`")?;
            self.expect(&Tok::If, "after `End`")?;
            (b, eb)
        };

        let arms = vec![
            MatchArm { pattern, guard: None, body },
            MatchArm { pattern: "_".to_string(), guard: None, body: else_body },
        ];
        Some(Stmt::Match { scrutinee, arms, line, if_let: true })
    }

    /// `Match <expr>` … `End Match`. Each arm is `pattern => body`, where the
    /// pattern is raw Rust and the body is one inline statement or an indented
    /// block running until the next arm (a line with `=>`) or `End Match`.
    fn parse_match(&mut self) -> Option<Stmt> {
        let line = self.line();
        self.expect(&Tok::Match, "")?;
        let scrutinee = self.parse_expr()?;
        self.expect(&Tok::Newline, "after the `Match` expression")?;

        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End | Tok::Eof) {
                break;
            }
            let pattern = self.parse_pattern()?;
            // Optional guard: `n If n < 0 =>`.
            let guard = if self.eat(&Tok::If) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&Tok::FatArrow, "after the pattern — every arm is `pattern => body`")?;
            // A body is either one statement on the same line, or an indented
            // block on the following lines (terminated by the next arm / `End`).
            let body = if matches!(self.peek(), Tok::Newline | Tok::Eof) {
                self.parse_arm_body()?
            } else {
                let mut b = vec![Stmt::LineMark(self.line()), self.parse_stmt()?];
                b.append(&mut self.dim_overflow);
                // A one-line arm may end with a trailing comment (`=> foo   ' note`);
                // consume it so it isn't mistaken for the next arm's pattern.
                if let Tok::Comment(text) = self.peek().clone() {
                    self.advance();
                    b.push(Stmt::Comment(text));
                }
                b
            };
            arms.push(MatchArm { pattern, guard, body });
        }

        self.expect(&Tok::End, "to close the `Match`")?;
        self.expect(&Tok::Match, "after `End`")?;
        Some(Stmt::Match { scrutinee, arms, line, if_let: false })
    }

    /// Capture a match-arm pattern as raw Rust text: every token up to the guard
    /// `If` or the `=>`. Tokens are space-joined, which is valid Rust for the
    /// whole pattern grammar (`Ok ( n )`, `1 ..= 10`, `1 | 2`, `Point { x , y }`).
    fn parse_pattern(&mut self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        while !matches!(
            self.peek(),
            Tok::FatArrow | Tok::If | Tok::Newline | Tok::Eof
        ) {
            let t = self.advance();
            parts.push(pattern_tok_src(&t, self.line()));
        }
        if parts.is_empty() {
            self.diags
                .error_at(self.span(), self.line(), "Expected a pattern before `=>`.");
            return None;
        }
        Some(parts.join(" "))
    }

    /// Parse an arm's multi-line body: statements until the next arm (recognised
    /// by a top-level `=>` on the line) or `End Match`. Nested `Match`/`If`/loops
    /// are consumed by their own parsers, so their inner `=>` never confuse us.
    fn parse_arm_body(&mut self) -> Option<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End | Tok::Eof) || self.line_has_fat_arrow() {
                break;
            }
            if !matches!(self.peek(), Tok::Comment(_)) {
                stmts.push(Stmt::LineMark(self.line()));
            }
            stmts.push(self.parse_stmt()?);
            stmts.append(&mut self.dim_overflow);
            if let Tok::Comment(text) = self.peek().clone() {
                self.advance();
                stmts.push(Stmt::Comment(text));
            }
            if !matches!(self.peek(), Tok::Newline | Tok::Eof)
                && !matches!(self.peek(), Tok::End)
                && !self.line_has_fat_arrow()
            {
                self.diags.error_at(
                    self.span(),
                    self.line(),
                    format!("Expected end of line after statement, found {:?}.", self.peek()),
                );
                return None;
            }
        }
        Some(stmts)
    }

    /// Does the current line contain a top-level `=>` (i.e. it starts a new arm)?
    fn line_has_fat_arrow(&self) -> bool {
        let mut k = self.pos;
        while k < self.toks.len() {
            match &self.toks[k].tok {
                Tok::Newline | Tok::Eof => return false,
                Tok::FatArrow => return true,
                _ => k += 1,
            }
        }
        false
    }

    fn parse_for(&mut self) -> Option<Stmt> {
        self.expect(&Tok::For, "")?;
        if self.eat(&Tok::Each) {
            return self.parse_for_each();
        }
        let var = self.expect_ident("for the loop variable")?;
        self.expect(&Tok::Eq, "after the loop variable")?;
        let from = self.parse_expr()?;
        self.expect(&Tok::To, "in the `For` range")?;
        let to = self.parse_expr()?;
        let step = if self.eat(&Tok::Step) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&Tok::Newline, "after the `For` header")?;
        let body = self.parse_block()?;
        self.expect(&Tok::Next, "to close the `For` loop")?;
        // `Next i` — the trailing variable name is optional.
        if let Tok::Ident(_) = self.peek() {
            self.advance();
        }
        Some(Stmt::For {
            var,
            from,
            to,
            step,
            body,
        })
    }

    fn parse_do(&mut self) -> Option<Stmt> {
        let line = self.line();
        self.expect(&Tok::Do, "")?;

        // A condition may sit on the `Do` (pre-test) ...
        let pre = self.parse_loop_cond()?;
        self.expect(&Tok::Newline, "after the `Do` header")?;
        let body = self.parse_block()?;
        self.expect(&Tok::Loop, "to close the `Do` loop")?;
        // ... or on the `Loop` (post-test).
        let post = self.parse_loop_cond()?;

        match (pre, post) {
            // `Do While <expr> Is <pattern> … Loop` → `while let`. Desugar to an
            // infinite loop whose body is an `if let … else break`; the Rust
            // backend renders `loop { if let P = e { … } else { break } }` and the
            // others the equivalent `while(1) { match … _ => break }` — all correct.
            (Some(LoopHead::WhileLet { pattern, scrutinee }), None) => {
                let arms = vec![
                    MatchArm { pattern, guard: None, body },
                    MatchArm { pattern: "_".to_string(), guard: None, body: vec![Stmt::Break] },
                ];
                Some(Stmt::DoLoop {
                    cond: None,
                    body: vec![Stmt::Match { scrutinee, arms, line, if_let: true }],
                })
            }
            (Some(LoopHead::While(c)), None) => Some(Stmt::DoLoop { cond: Some(DoCond::PreWhile(c)), body }),
            (Some(LoopHead::Until(c)), None) => Some(Stmt::DoLoop { cond: Some(DoCond::PreUntil(c)), body }),
            (None, Some(LoopHead::While(c))) => Some(Stmt::DoLoop { cond: Some(DoCond::PostWhile(c)), body }),
            (None, Some(LoopHead::Until(c))) => Some(Stmt::DoLoop { cond: Some(DoCond::PostUntil(c)), body }),
            (None, None) => Some(Stmt::DoLoop { cond: None, body }),
            (None, Some(LoopHead::WhileLet { .. })) => {
                self.diags.error(line, "`Is` (while-let) works on `Do While`, not `Loop While`.");
                None
            }
            (Some(_), Some(_)) => {
                self.diags.error(
                    line,
                    "A `Do` loop can have a condition on the `Do` or the `Loop`, not both.",
                );
                None
            }
        }
    }

    /// Parse an optional `While c` / `Until c`; returns (is_while, cond).
    fn parse_loop_cond(&mut self) -> Option<Option<LoopHead>> {
        if self.eat(&Tok::While) {
            let e = self.parse_expr()?;
            // `Do While <expr> Is <pattern>` — VB-flavoured `while let`.
            if matches!(self.peek(), Tok::Ident(n) if n.eq_ignore_ascii_case("is")) {
                self.advance(); // Is
                let mut parts: Vec<String> = Vec::new();
                while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                    let t = self.advance();
                    parts.push(pattern_tok_src(&t, self.line()));
                }
                if parts.is_empty() {
                    self.diags.error_at(
                        self.span(),
                        self.line(),
                        "Expected a pattern after `Is` — e.g. `Do While q.Pop() Is Some(x)`.",
                    );
                    return None;
                }
                Some(Some(LoopHead::WhileLet { pattern: parts.join(" "), scrutinee: e }))
            } else {
                Some(Some(LoopHead::While(e)))
            }
        } else if self.eat(&Tok::Until) {
            Some(Some(LoopHead::Until(self.parse_expr()?)))
        } else {
            Some(None)
        }
    }

    fn parse_for_each(&mut self) -> Option<Stmt> {
        let var1 = self.expect_ident("for the loop item")?;
        let var2 = if self.eat(&Tok::Comma) {
            Some(self.expect_ident("for the second loop item")?)
        } else {
            None
        };
        self.expect(&Tok::In, "after the `For Each` variables")?;
        let iter = self.parse_expr()?;
        self.expect(&Tok::Newline, "after the `For Each` header")?;
        let body = self.parse_block()?;
        self.expect(&Tok::Next, "to close the `For Each` loop")?;
        if let Tok::Ident(_) = self.peek() {
            self.advance();
        }
        Some(Stmt::ForEach {
            var1,
            var2,
            iter,
            body,
        })
    }

    // ----- Expressions (precedence climbing) -----

    fn parse_expr(&mut self) -> Option<Expr> {
        // `Await <expr>` — a prefix that wraps the awaited expression (a stdlib
        // call). Only meaningful in a Window event; the GUI codegen handles it.
        if matches!(self.peek(), Tok::Await) {
            let start = self.span();
            self.advance();
            let inner = self.parse_expr()?;
            let span = start.to(inner.span);
            return Some(ExprKind::Await(Box::new(inner)).at(span));
        }
        // `Raw F()` — contextual prefix (only when the next token is a name).
        if matches!(self.peek(), Tok::Ident(w) if w.eq_ignore_ascii_case("raw"))
            && matches!(self.peek2(), Tok::Ident(_))
        {
            let start = self.span();
            self.advance();
            let inner = self.parse_expr()?;
            let span = start.to(inner.span);
            return Some(ExprKind::Raw(Box::new(inner)).at(span));
        }
        self.parse_or()
    }

    // Logical operators bind looser than comparison (as in both VB and Rust);
    // tightness: Not > And > Xor > Or. They are short-circuit (&&/||), per Rust.
    fn parse_or(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_xor()?;
        while matches!(self.peek(), Tok::Or) {
            self.advance();
            let rhs = self.parse_xor()?;
            lhs = bin(BinOp::Or, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_xor(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_and()?;
        while matches!(self.peek(), Tok::Xor) {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = bin(BinOp::Xor, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_and(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_not()?;
        while matches!(self.peek(), Tok::And) {
            self.advance();
            let rhs = self.parse_not()?;
            lhs = bin(BinOp::And, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_not(&mut self) -> Option<Expr> {
        if matches!(self.peek(), Tok::Not) {
            let start = self.span();
            self.advance();
            let inner = self.parse_not()?;
            let span = start.to(inner.span);
            return Some(ExprKind::Not(Box::new(inner)).at(span));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_concat()?;
        while let Some(op) = self.comparison_op() {
            self.advance();
            let rhs = self.parse_concat()?;
            lhs = bin(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn comparison_op(&self) -> Option<BinOp> {
        match self.peek() {
            Tok::Eq => Some(BinOp::Eq),
            Tok::Ne => Some(BinOp::Ne),
            Tok::Lt => Some(BinOp::Lt),
            Tok::Gt => Some(BinOp::Gt),
            Tok::Le => Some(BinOp::Le),
            Tok::Ge => Some(BinOp::Ge),
            _ => None,
        }
    }

    fn parse_concat(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_add()?;
        while matches!(self.peek(), Tok::Amp) {
            self.advance();
            let rhs = self.parse_add()?;
            lhs = bin(BinOp::Concat, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_add(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = bin(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_mul(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Mod => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = bin(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        if matches!(self.peek(), Tok::Minus) {
            let start = self.span();
            self.advance();
            // `^` binds tighter than unary minus, so negate a whole power.
            let e = self.parse_unary()?;
            let span = start.to(e.span);
            let inner_span = e.span;
            return Some(match e.kind {
                ExprKind::Int(n) => ExprKind::Int(-n).at(span),
                ExprKind::Float(f) => ExprKind::Float(-f).at(span),
                other => bin(BinOp::Sub, ExprKind::Int(0).at(start), other.at(inner_span)),
            });
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Option<Expr> {
        let base = self.parse_primary()?;
        if matches!(self.peek(), Tok::Caret) {
            self.advance();
            // Right operand via parse_unary so `2 ^ -3` works.
            let exp = self.parse_unary()?;
            Some(bin(BinOp::Pow, base, exp))
        } else {
            Some(base)
        }
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let mut e = self.parse_atom()?;
        // Postfix: chained `.method(args)` calls and the `?` operator.
        loop {
            match self.peek() {
                Tok::Dot => {
                    self.advance();
                    // `expr.0` — tuple element access.
                    if let Tok::Int(n) = self.peek() {
                        let n = *n as usize;
                        self.advance();
                        let span = e.span.to(self.prev_span());
                        e = ExprKind::TupleIndex(Box::new(e), n).at(span);
                        continue;
                    }
                    let member = self.expect_ident("after `.`")?;
                    if matches!(self.peek(), Tok::LParen) {
                        // method call: expr.method(args)
                        self.advance();
                        let mut args = Vec::new();
                        if !matches!(self.peek(), Tok::RParen) {
                            loop {
                                args.push(self.parse_expr()?);
                                if !self.eat(&Tok::Comma) {
                                    break;
                                }
                                if matches!(self.peek(), Tok::RParen) {
                                    break;
                                }
                            }
                        }
                        self.expect(&Tok::RParen, "to close the method arguments")?;
                        let span = e.span.to(self.prev_span());
                        e = ExprKind::MethodCall {
                            recv: Box::new(e),
                            method: member,
                            args,
                        }
                        .at(span);
                    } else {
                        // field access: expr.field
                        let span = e.span.to(self.prev_span());
                        e = ExprKind::Field(Box::new(e), member).at(span);
                    }
                }
                Tok::Question => {
                    let span = self.span();
                    let line = self.line();
                    self.advance();
                    self.diags.error_at(
                        span,
                        line,
                        "`?` is gone. Errors propagate automatically — just write the call. \
                         To intercept it, use `Handle err` … `End Handle`. To take the \
                         `Result` as a value, use `Raw F()`.",
                    );
                }
                // `expr[index]` — array/Vec indexing (chainable for 2D).
                Tok::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Tok::RBracket, "to close the index")?;
                    let span = e.span.to(self.prev_span());
                    e = ExprKind::Index(Box::new(e), Box::new(index)).at(span);
                }
                _ => break,
            }
        }
        Some(e)
    }

    fn parse_atom(&mut self) -> Option<Expr> {
        // An inline Rust block.
        if let Tok::InlineRust(raw) = self.peek().clone() {
            let span = self.span();
            self.advance();
            return Some(ExprKind::InlineRust(raw).at(span));
        }
        // A `Text … End Text` block — a multi-line string literal. Dedented
        // here (common leading indentation stripped), then an ordinary string
        // from every other angle: `&` concatenation, `.to_string()` coercion.
        if let Tok::TextBlock { body, terminated } = self.peek().clone() {
            let opened_at = self.line();
            let span = self.span();
            self.advance();
            if !terminated {
                self.diags.error_at(
                    span,
                    opened_at,
                    "This `Text` block never ends — close it with `End Text` on \
                     its own line.",
                );
                return None;
            }
            return Some(ExprKind::Str(dedent_text_block(&body)).at(span));
        }
        // A backtick-quoted column name in a dataframe formula — sugar for
        // `Col("Unit Price")`; the resolver lowers both to polars `col(...)`.
        if let Tok::Backtick(name) = self.peek().clone() {
            let span = self.span();
            self.advance();
            return Some(
                ExprKind::Call {
                    name: "Col".to_string(),
                    args: vec![ExprKind::Str(name).at(span)],
                }
                .at(span),
            );
        }
        // An inline Python block (run via pyo3; typed by the surrounding `As T`,
        // or an opaque `PyObject` handle when untyped).
        if let Tok::InlinePython { args, body } = self.peek().clone() {
            let span = self.span();
            self.advance();
            return Some(
                ExprKind::InlinePython {
                    inputs: split_py_args(&args),
                    body,
                }
                .at(span),
            );
        }
        // A closure: `|x| body` (or `|| body`).
        if matches!(self.peek(), Tok::Pipe) {
            let start = self.span();
            self.advance();
            let mut params = Vec::new();
            if !matches!(self.peek(), Tok::Pipe) {
                loop {
                    params.push(self.expect_ident("for the closure parameter")?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
            }
            self.expect(&Tok::Pipe, "to close the closure parameters")?;
            let body = self.parse_expr()?;
            let span = start.to(body.span);
            return Some(
                ExprKind::Closure {
                    params,
                    body: Box::new(body),
                    by_ref_params: false,
                }
                .at(span),
            );
        }
        let start = self.span();
        match self.peek().clone() {
            Tok::Int(n) => {
                self.advance();
                Some(ExprKind::Int(n).at(start))
            }
            Tok::Float(f) => {
                self.advance();
                Some(ExprKind::Float(f).at(start))
            }
            Tok::Str(s) => {
                self.advance();
                Some(ExprKind::Str(s).at(start))
            }
            Tok::True => {
                self.advance();
                Some(ExprKind::Bool(true).at(start))
            }
            Tok::False => {
                self.advance();
                Some(ExprKind::Bool(false).at(start))
            }
            Tok::Ident(name) => {
                self.advance();
                // A name followed by `{` is a struct literal.
                if matches!(self.peek(), Tok::LBrace) {
                    self.advance();
                    let mut fields = Vec::new();
                    if !matches!(self.peek(), Tok::RBrace) {
                        loop {
                            let fname = self.expect_ident("for the field")?;
                            self.expect(&Tok::Colon, "after the field name")?;
                            let fval = self.parse_expr()?;
                            fields.push((fname, fval));
                            if !self.eat(&Tok::Comma) {
                                break;
                            }
                            // Allow a trailing comma before `}`.
                            if matches!(self.peek(), Tok::RBrace) {
                                break;
                            }
                        }
                    }
                    self.expect(&Tok::RBrace, "to close the struct literal")?;
                    return Some(ExprKind::StructLit { name, fields }.at(start.to(self.prev_span())));
                }
                // A name followed by `(` is a function call.
                if matches!(self.peek(), Tok::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Tok::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.eat(&Tok::Comma) {
                                break;
                            }
                            if matches!(self.peek(), Tok::RParen) {
                                break;
                            }
                        }
                    }
                    self.expect(&Tok::RParen, "to close the call arguments")?;
                    Some(ExprKind::Call { name, args }.at(start.to(self.prev_span())))
                } else {
                    Some(ExprKind::Ident(name).at(start))
                }
            }
            // `[a, b, …]` — an inline list literal (primary position). Postfix
            // `expr[i]` indexing is handled in the suffix loop, so no clash.
            Tok::LBracket => {
                self.advance();
                let mut elems = Vec::new();
                if !matches!(self.peek(), Tok::RBracket) {
                    loop {
                        elems.push(self.parse_expr()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                        // Allow a trailing comma before `]` (common when the list
                        // spans lines).
                        if matches!(self.peek(), Tok::RBracket) {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RBracket, "to close the list literal")?;
                Some(ExprKind::List(elems).at(start.to(self.prev_span())))
            }
            Tok::LParen => {
                self.advance();
                // `()` — the unit value (e.g. `Ok(())` in a `Result<()>` function).
                if matches!(self.peek(), Tok::RParen) {
                    self.advance();
                    return Some(ExprKind::Tuple(Vec::new()).at(start.to(self.prev_span())));
                }
                let first = self.parse_expr()?;
                if matches!(self.peek(), Tok::Comma) {
                    // A tuple: (a, b, …)
                    let mut elems = vec![first];
                    while self.eat(&Tok::Comma) {
                        // allow a trailing comma
                        if matches!(self.peek(), Tok::RParen) {
                            break;
                        }
                        elems.push(self.parse_expr()?);
                    }
                    self.expect(&Tok::RParen, "to close the tuple")?;
                    Some(ExprKind::Tuple(elems).at(start.to(self.prev_span())))
                } else {
                    self.expect(&Tok::RParen, "to close `(`")?;
                    Some(first)
                }
            }
            other => {
                self.diags
                    .error_at(self.span(), self.line(), format!("Expected an expression, found {:?}.", other));
                None
            }
        }
    }
}

/// The shape declared in a `Dim` name's parentheses.
enum DimSpec {
    None,
    Empty1D,           // x()
    Empty2D,           // x(,)
    Fixed1D(usize),    // x(N)
    Fixed2D(usize, usize), // x(R, C)
}

/// A `Log.<Level>` severity word, if `tok` names one (`Info`/`Warn`/`Error`/
/// `Debug`, case-insensitive). None otherwise, so `log.Push(x)` isn't a log.
fn log_level(tok: &Tok) -> Option<LogLevel> {
    let Tok::Ident(w) = tok else { return None };
    match w.to_ascii_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" => Some(LogLevel::Warn),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}

fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let span = lhs.span.to(rhs.span);
    ExprKind::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
    .at(span)
}

/// Split the raw text inside `Python(…)` into the variable names passed in.
/// Slice 2 inputs are bare identifiers (`Python(df, count)`); commas separate them.
/// Assemble a `Text … End Text` body into its string value: drop the (blank)
/// remainder of the opening line, strip the common leading indentation — so
/// the block indents with the surrounding code without the indent leaking
/// into the string — and join with `\n`. Blank lines survive; there is no
/// trailing newline (`Debug.Print` adds its own).
fn dedent_text_block(raw: &str) -> String {
    let lines: Vec<&str> = raw
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    // The capture starts just after the word `Text`, so the first "line" is
    // the blank tail of the opening line — drop it.
    let lines = match lines.split_first() {
        Some((first, rest)) if first.trim().is_empty() => rest,
        _ => &lines[..],
    };
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| if l.trim().is_empty() { "" } else { &l[indent..] })
        .collect::<Vec<&str>>()
        .join("\n")
}

fn split_py_args(args: &str) -> Vec<String> {
    args.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The head of a `Do`/`Loop` condition: `While c`, `Until c`, or the `while let`
/// form `While c Is <pattern>` (desugared to a loop + `if let … else break`).
enum LoopHead {
    While(Expr),
    Until(Expr),
    WhileLet { pattern: String, scrutinee: Expr },
}

/// Render one token of a match-arm pattern as Rust source. Patterns pass through
/// verbatim, so this is a faithful surface form for the token kinds patterns use.
fn pattern_tok_src(t: &Tok, line: usize) -> String {
    match t {
        Tok::Ident(s) => s.clone(),
        Tok::Int(n) => n.to_string(),
        Tok::Float(f) => format!("{f}"),
        Tok::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Tok::True => "true".to_string(),
        Tok::False => "false".to_string(),
        Tok::Minus => "-".to_string(),
        Tok::LParen => "(".to_string(),
        Tok::RParen => ")".to_string(),
        Tok::LBrace => "{".to_string(),
        Tok::RBrace => "}".to_string(),
        Tok::Comma => ",".to_string(),
        Tok::Pipe => "|".to_string(),
        Tok::DotDotEq => "..=".to_string(),
        Tok::DotDot => "..".to_string(),
        // A `.` in a pattern is always a path separator (enum variant like
        // `Color.Red`) — there are no value field-accesses in pattern position.
        Tok::Dot => "::".to_string(),
        Tok::Colon => ":".to_string(),
        Tok::Amp => "&".to_string(),
        // Anything else isn't part of a pattern — let it through as a best-effort
        // token so rustc reports a precise location if it really is a mistake.
        other => {
            let _ = line;
            format!("{other:?}")
        }
    }
}
