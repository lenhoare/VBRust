//! Recursive-descent parser: tokens in, AST out.
//!
//! On an unexpected token the parser records an `✘` diagnostic and unwinds via
//! `Option`/`?`, so a malformed file produces a clear message instead of a panic.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::lexer::{Tok, Token};

pub fn parse(tokens: Vec<Token>, diags: &mut Diagnostics) -> Program {
    let mut p = Parser {
        toks: tokens,
        pos: 0,
        diags,
    };
    p.parse_program()
}

struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    diags: &'a mut Diagnostics,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn line(&self) -> usize {
        self.toks[self.pos].line
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
            self.diags.error(
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
            self.diags.error(
                self.line(),
                format!("Expected a name {}, found {:?}.", ctx, self.peek()),
            );
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
        let mut constants = Vec::new();
        let mut uses = Vec::new();
        let mut windows = Vec::new();
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
                Tok::Function => match self.parse_function(false, false) {
                    Some(f) => functions.push(f),
                    None => break, // error already recorded
                },
                Tok::Sub => match self.parse_function(false, true) {
                    Some(f) => functions.push(f),
                    None => break,
                },
                Tok::Type => match self.parse_struct(false) {
                    Some(s) => structs.push(s),
                    None => break,
                },
                Tok::Const => match self.parse_const(false) {
                    Some(c) => constants.push(c),
                    None => break,
                },
                Tok::Public | Tok::Private => {
                    let public = matches!(self.peek(), Tok::Public);
                    self.advance();
                    match self.peek() {
                        Tok::Function => match self.parse_function(public, false) {
                            Some(f) => functions.push(f),
                            None => break,
                        },
                        Tok::Sub => match self.parse_function(public, true) {
                            Some(f) => functions.push(f),
                            None => break,
                        },
                        Tok::Type => match self.parse_struct(public) {
                            Some(s) => structs.push(s),
                            None => break,
                        },
                        Tok::Const => match self.parse_const(public) {
                            Some(c) => constants.push(c),
                            None => break,
                        },
                        _ => {
                            self.diags.error(
                                self.line(),
                                "Module-level variables (global state) aren't supported. Rust \
                                 avoids global mutable state because it makes data races easy to \
                                 write by accident. Instead, pass the value into the functions \
                                 that need it — as a `ByRef` parameter when they must change it — \
                                 or wrap related state in a struct (`Type`) and give it methods. \
                                 (Module-level `Const` is fine — it's immutable and shared safely.)",
                            );
                            break;
                        }
                    }
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Window") => {
                    if let Some(win) = self.parse_window() {
                        windows.push(win);
                    } else {
                        break;
                    }
                }
                Tok::Ident(w) if w == "Option" => {
                    self.diags.error(
                        self.line(),
                        "`Option` directives (Option Base, Option Explicit, …) aren't \
                         supported and aren't needed — Rust is always zero-indexed and \
                         always explicit about types.",
                    );
                    break;
                }
                other => {
                    self.diags.error(
                        self.line(),
                        format!(
                            "Top level may only contain functions, found {:?}. \
                             Every VBR program starts at `Function Main()`.",
                            other
                        ),
                    );
                    break;
                }
            }
        }
        Program {
            leading_comments: top_comments,
            uses,
            constants,
            structs,
            functions,
            windows,
        }
    }

    /// `Use <crate> <version>` — the line was captured raw by the lexer; split it
    /// into the crate name and its version requirement.
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
        let version: String = parts.collect::<Vec<_>>().join(" ");
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
                self.diags.error(
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

    fn parse_function(&mut self, public: bool, is_sub: bool) -> Option<Function> {
        let line = self.line();
        if is_sub {
            self.expect(&Tok::Sub, "to start a sub")?;
            self.diags.warn_once(
                "sub-is-function",
                line,
                "`Sub` works, but in VBR it's just a `Function` with no return value — both \
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
                self.diags.error(
                    self.line(),
                    "A `Sub` returns nothing — to return a value, use `Function … As T`.",
                );
                return None;
            }
            None
        } else if self.eat(&Tok::As) {
            // `Function Foo() As Long` / `As Result<Long>` / `As Option<String>`
            Some(self.parse_decl_type()?)
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
                    .error(self.line(), format!("Expected `{}`, found {:?}.", name, other));
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
                    .error(self.line(), format!("Expected a string {}, found {:?}.", ctx, other));
                None
            }
        }
    }

    fn parse_window(&mut self) -> Option<Window> {
        self.advance(); // `Window`
        let name = self.expect_ident("for the window name")?;
        self.expect(&Tok::Newline, "after the window name")?;

        let mut title = None;
        let mut state = Vec::new();
        let mut view = None;
        let mut events = Vec::new();

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Tok::End => {
                    self.advance();
                    self.expect_kw_ident("Window")?;
                    self.eat(&Tok::Newline);
                    break;
                }
                Tok::Ident(w) if w.eq_ignore_ascii_case("Title") => {
                    self.advance();
                    title = Some(self.expect_string("after `Title`")?);
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
                other => {
                    self.diags.error(
                        self.line(),
                        format!(
                            "Unexpected {:?} inside a Window — expected Title, State, View, \
                             Event, or `End Window`.",
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
                    .error(self.line(), "A Window needs a `View` block.");
                return None;
            }
        };
        Some(Window {
            name,
            title,
            state,
            view,
            events,
        })
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
            if !matches!(self.peek(), Tok::Dim) {
                self.diags
                    .error(self.line(), "A `State` block may only contain `Dim` declarations.");
                return None;
            }
            match self.parse_dim()? {
                Stmt::Dim {
                    name,
                    ty: DeclType::Plain(t),
                    init: Some(init),
                    ..
                } => fields.push(StateField { name, ty: t, init }),
                _ => {
                    self.diags.error(
                        self.line(),
                        "A State field must be a simple typed value with an initial value, \
                         e.g. `Dim count As Integer = 0`.",
                    );
                    return None;
                }
            }
            self.eat(&Tok::Newline);
        }
        Some(fields)
    }

    fn parse_view_node(&mut self) -> Option<ViewNode> {
        self.skip_newlines();
        // `Match` lexes to a keyword token, so handle it before widget names.
        if matches!(self.peek(), Tok::Match) {
            return self.parse_view_match();
        }
        let kw = match self.peek().clone() {
            Tok::Ident(w) => w,
            other => {
                self.diags
                    .error(self.line(), format!("Expected a widget, found {:?}.", other));
                return None;
            }
        };
        match kw.to_ascii_lowercase().as_str() {
            "column" => {
                self.advance();
                self.eat(&Tok::Newline);
                Some(ViewNode::Column(self.parse_view_children("Column")?))
            }
            "row" => {
                self.advance();
                self.eat(&Tok::Newline);
                Some(ViewNode::Row(self.parse_view_children("Row")?))
            }
            "text" => {
                self.advance();
                let e = self.parse_expr()?;
                self.eat(&Tok::Newline);
                Some(ViewNode::Text(e))
            }
            "button" => {
                self.advance();
                let label = self.parse_expr()?;
                self.eat(&Tok::Newline);
                let mut on_click = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Click")?;
                            on_click = Some(self.expect_ident("for the click event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("Button")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error(
                                self.line(),
                                format!(
                                    "Inside a Button expected `On Click <event>` or `End Button`, \
                                     found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::Button { label, on_click })
            }
            "textinput" => {
                self.advance();
                let placeholder = self.parse_expr()?;
                // The bound state field follows the placeholder: `TextInput "p", name`.
                self.expect(&Tok::Comma, "after the placeholder — `TextInput \"hint\", field`")?;
                let value = self.expect_ident("for the bound state field")?;
                self.eat(&Tok::Newline);
                let mut on_input = None;
                loop {
                    self.skip_newlines();
                    match self.peek() {
                        Tok::On => {
                            self.advance();
                            self.expect_kw_ident("Input")?;
                            on_input = Some(self.expect_ident("for the input event")?);
                            self.eat(&Tok::Newline);
                        }
                        Tok::End => {
                            self.advance();
                            self.expect_kw_ident("TextInput")?;
                            self.eat(&Tok::Newline);
                            break;
                        }
                        other => {
                            self.diags.error(
                                self.line(),
                                format!(
                                    "Inside a TextInput expected `On Input <event>` or \
                                     `End TextInput`, found {:?}.",
                                    other
                                ),
                            );
                            return None;
                        }
                    }
                }
                Some(ViewNode::TextInput { placeholder, value, on_input })
            }
            other => {
                self.diags.error(
                    self.line(),
                    format!(
                        "Unknown widget `{}` (have: Column, Row, Text, Button, TextInput, Match).",
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

    fn parse_view_children(&mut self, container: &str) -> Option<Vec<ViewNode>> {
        let mut children = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::End) {
                self.advance();
                self.expect_kw_ident(container)?;
                self.eat(&Tok::Newline);
                break;
            }
            children.push(self.parse_view_node()?);
        }
        Some(children)
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

    /// Parse a comma-separated parameter list, assuming the opening `(` is already
    /// consumed, and consume the closing `)`.
    fn parse_params_until_rparen(&mut self) -> Option<Vec<Param>> {
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
        Some(params)
    }

    /// Emit the "use DateTime" redirect for a `Date` used in type position.
    fn reject_date(&mut self, line: usize) {
        self.diags.error(
            line,
            "Date isn't a built-in VBR type — a bare date with no calendar semantics is \
             just a number in disguise. Use `DateTime` from the standard library: \
             `Dim now As DateTime = DateTime.Now()`, then `.AddDays(n)`, `.Format(...)`, etc.",
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
            let s = self.parse_stmt()?;
            stmts.push(s);

            if let Tok::Comment(text) = self.peek().clone() {
                self.advance();
                stmts.push(Stmt::Comment(text));
            }

            if !matches!(self.peek(), Tok::Newline | Tok::Eof) && !self.at_block_end() {
                self.diags.error(
                    self.line(),
                    format!("Expected end of line after statement, found {:?}.", self.peek()),
                );
                return None;
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
                self.diags.error(
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
                self.diags.error(
                    self.line(),
                    "Declare constants at the top of the file (module level), not inside a function.",
                );
                None
            }
            Tok::ReDim => {
                self.diags.error(
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
                self.diags.error(
                    self.line(),
                    "`With` blocks aren't supported — write the variable name out each time \
                     (e.g. `p.x = 1` / `p.y = 2`, not `With p` … `.x = 1`). It's a little more \
                     typing but far clearer about what you're touching.",
                );
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
                        self.diags.error(
                            self.line(),
                            format!("`Exit {:?}` is not supported — use `Exit Do`, `Exit For`, or `Exit Function`.", other),
                        );
                        None
                    }
                }
            }
            Tok::On => {
                self.diags.error(
                    self.line(),
                    "`On Error` is not supported. Rust signals failure through return values, \
                     not jumps. Make the function return `As Result<T>`, `Return Err(\"...\")` on \
                     failure, and handle it at the call site with the `?` operator or \
                     `Match` over `Ok`/`Err`.",
                );
                // Swallow the rest of the line so we don't cascade more errors.
                while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                    self.advance();
                }
                None
            }
            Tok::Ident(name) => self.parse_ident_stmt(name),
            other => {
                self.diags
                    .error(self.line(), format!("Unexpected {:?} at start of statement.", other));
                None
            }
        }
    }

    fn parse_dim(&mut self) -> Option<Stmt> {
        let line = self.line();
        self.expect(&Tok::Dim, "")?;
        let name = self.expect_ident("after `Dim`")?;

        // `Dim a, b = expr` destructures a tuple.
        if matches!(self.peek(), Tok::Comma) {
            let mut names = vec![name];
            while self.eat(&Tok::Comma) {
                names.push(self.expect_ident("for the destructured name")?);
            }
            self.expect(&Tok::Eq, "in a tuple destructuring (`Dim a, b = …`)")?;
            let value = self.parse_expr()?;
            return Some(Stmt::DestructureDim { names, value });
        }

        // `Dim name = Rust … End Rust` — an opaque handle. The only `As`-less
        // single `Dim`: the type is whatever Rust infers, hidden from VBR.
        if self.eat(&Tok::Eq) {
            if let Tok::InlineRust(raw) = self.peek().clone() {
                self.advance();
                return Some(Stmt::HandleDim { name, raw, line });
            }
            self.diags.error(
                line,
                "A `Dim` needs a type: `Dim x As Long`. The one exception is \
                 `Dim h = Rust … End Rust`, which makes an opaque Rust handle whose \
                 type Rust infers — VBR can pass it back into another `Rust` block but \
                 can't use it as a value.",
            );
            return None;
        }

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
            DeclType::Vec(_) | DeclType::Map(..) | DeclType::Result(_) | DeclType::Option(_) => {
                if self.eat(&Tok::Eq) {
                    Some(self.parse_expr()?)
                } else {
                    None
                }
            }
            // Fixed arrays are auto-zeroed.
            DeclType::Array(..) | DeclType::Array2D(..) => None,
        };
        Some(Stmt::Dim {
            name,
            ty,
            init,
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
            self.diags.error(
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
        // `New` is a VB-ism with no meaning in VBR (Rust has no uninitialised
        // objects) — accept it out of habit, but nudge toward dropping it.
        if self.eat(&Tok::New) {
            self.diags.warn(
                self.line(),
                "`New` isn't needed in VBR — a value is created by its declaration. \
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
                    self.expect(&Tok::Gt, "to close `Result<...>`")?;
                    Some(DeclType::Result(Box::new(t)))
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
        self.expect(&Tok::Set, "")?;
        let mutable = self.eat(&Tok::Mut);
        let name = self.expect_ident("after `Set`")?;
        self.expect(&Tok::Eq, "in a `Set` borrow")?;
        let value = self.parse_expr()?;
        Some(Stmt::Set {
            name,
            mutable,
            value,
        })
    }

    /// An identifier at statement start is either `Debug.Print expr` or `name = expr`.
    fn parse_ident_stmt(&mut self, name: String) -> Option<Stmt> {
        if name.eq_ignore_ascii_case("Debug") {
            self.advance(); // Debug
            self.expect(&Tok::Dot, "after `Debug`")?;
            let method = self.expect_ident("after `Debug.`")?;
            if !method.eq_ignore_ascii_case("Print") {
                self.diags.error(
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
                "MsgBox has no window in a terminal app, so VBR prints it to the terminal \
                 (like Debug.Print). InputBox reads a line of input back.",
            );
            let value = self.parse_expr()?;
            return Some(Stmt::Print(value));
        }

        // Parse a place expression (Ident or `a.field`) or a call. `parse_primary`
        // stops before binary operators, so a top-level `=` isn't mistaken for the
        // equality operator.
        let target = self.parse_primary()?;
        if self.eat(&Tok::Eq) {
            let value = self.parse_expr()?;
            Some(Stmt::Assign { target, value, op: None })
        } else if let Some(op) = self.compound_assign_op() {
            self.advance();
            let value = self.parse_expr()?;
            Some(Stmt::Assign { target, value, op: Some(op) })
        } else {
            Some(Stmt::Expr(target))
        }
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
        self.expect(&Tok::If, "")?;
        let cond = self.parse_expr()?;
        self.expect(&Tok::Then, "after the `If` condition")?;
        // Single-line form: `If cond Then <stmt> [Else <stmt>]` — a statement
        // follows `Then` on the same line, and there is no `End If`.
        if !matches!(self.peek(), Tok::Newline) {
            let then_stmt = self.parse_stmt()?;
            let else_body = if self.eat(&Tok::Else) {
                Some(vec![self.parse_stmt()?])
            } else {
                None
            };
            return Some(Stmt::If {
                branches: vec![(cond, vec![then_stmt])],
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
                vec![self.parse_stmt()?]
            };
            arms.push(MatchArm { pattern, guard, body });
        }

        self.expect(&Tok::End, "to close the `Match`")?;
        self.expect(&Tok::Match, "after `End`")?;
        Some(Stmt::Match { scrutinee, arms, line })
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
                .error(self.line(), "Expected a pattern before `=>`.");
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
            stmts.push(self.parse_stmt()?);
            if let Tok::Comment(text) = self.peek().clone() {
                self.advance();
                stmts.push(Stmt::Comment(text));
            }
            if !matches!(self.peek(), Tok::Newline | Tok::Eof)
                && !matches!(self.peek(), Tok::End)
                && !self.line_has_fat_arrow()
            {
                self.diags.error(
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

        let cond = match (pre, post) {
            (Some((true, c)), None) => Some(DoCond::PreWhile(c)),
            (Some((false, c)), None) => Some(DoCond::PreUntil(c)),
            (None, Some((true, c))) => Some(DoCond::PostWhile(c)),
            (None, Some((false, c))) => Some(DoCond::PostUntil(c)),
            (None, None) => None,
            (Some(_), Some(_)) => {
                self.diags.error(
                    line,
                    "A `Do` loop can have a condition on the `Do` or the `Loop`, not both.",
                );
                None
            }
        };
        Some(Stmt::DoLoop { cond, body })
    }

    /// Parse an optional `While c` / `Until c`; returns (is_while, cond).
    fn parse_loop_cond(&mut self) -> Option<Option<(bool, Expr)>> {
        if self.eat(&Tok::While) {
            Some(Some((true, self.parse_expr()?)))
        } else if self.eat(&Tok::Until) {
            Some(Some((false, self.parse_expr()?)))
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
            self.advance();
            let inner = self.parse_not()?;
            return Some(Expr::Not(Box::new(inner)));
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
            self.advance();
            // `^` binds tighter than unary minus, so negate a whole power.
            let e = self.parse_unary()?;
            return Some(match e {
                Expr::Int(n) => Expr::Int(-n),
                Expr::Float(f) => Expr::Float(-f),
                other => bin(BinOp::Sub, Expr::Int(0), other),
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
                        e = Expr::TupleIndex(Box::new(e), n);
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
                            }
                        }
                        self.expect(&Tok::RParen, "to close the method arguments")?;
                        e = Expr::MethodCall {
                            recv: Box::new(e),
                            method: member,
                            args,
                        };
                    } else {
                        // field access: expr.field
                        e = Expr::Field(Box::new(e), member);
                    }
                }
                Tok::Question => {
                    self.advance();
                    e = Expr::Try(Box::new(e));
                }
                // `expr[index]` — array/Vec indexing (chainable for 2D).
                Tok::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Tok::RBracket, "to close the index")?;
                    e = Expr::Index(Box::new(e), Box::new(index));
                }
                _ => break,
            }
        }
        Some(e)
    }

    fn parse_atom(&mut self) -> Option<Expr> {
        // An inline Rust block.
        if let Tok::InlineRust(raw) = self.peek().clone() {
            self.advance();
            return Some(Expr::InlineRust(raw));
        }
        // A closure: `|x| body` (or `|| body`).
        if matches!(self.peek(), Tok::Pipe) {
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
            return Some(Expr::Closure {
                params,
                body: Box::new(body),
            });
        }
        match self.peek().clone() {
            Tok::Int(n) => {
                self.advance();
                Some(Expr::Int(n))
            }
            Tok::Float(f) => {
                self.advance();
                Some(Expr::Float(f))
            }
            Tok::Str(s) => {
                self.advance();
                Some(Expr::Str(s))
            }
            Tok::True => {
                self.advance();
                Some(Expr::Bool(true))
            }
            Tok::False => {
                self.advance();
                Some(Expr::Bool(false))
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
                        }
                    }
                    self.expect(&Tok::RBrace, "to close the struct literal")?;
                    return Some(Expr::StructLit { name, fields });
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
                        }
                    }
                    self.expect(&Tok::RParen, "to close the call arguments")?;
                    Some(Expr::Call { name, args })
                } else {
                    Some(Expr::Ident(name))
                }
            }
            Tok::LParen => {
                self.advance();
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
                    Some(Expr::Tuple(elems))
                } else {
                    self.expect(&Tok::RParen, "to close `(`")?;
                    Some(first)
                }
            }
            other => {
                self.diags
                    .error(self.line(), format!("Expected an expression, found {:?}.", other));
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

fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
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
