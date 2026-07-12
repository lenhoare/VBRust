//! Completion — what can the cursor see?
//!
//! `completions_at(source, offset)` answers the editor's completion request
//! from the compiler's own knowledge, in two contexts:
//!
//! - **After a dot** (`x.`, `Http.`, `Color.`): members of the receiver. A
//!   typed variable offers the methods of its type (strings and numbers get
//!   the curated *real Rust* method set — the same names that pass through to
//!   rustc); a stdlib namespace offers its functions with VB-facing
//!   signatures; an enum offers its variants; a user struct its fields and
//!   methods.
//! - **Bare position**: variables in scope (from the resolver's symbol table,
//!   scoped to the enclosing function), the program's functions, constants,
//!   enums, stdlib namespaces, and the statement keywords.
//!
//! It works on half-typed files by design — the parser's error recovery keeps
//! the rest of the file analysed, so the symbol table still knows `x`'s type
//! while the line with `x.` is broken.

use crate::ast::{DeclType, Function, Program, Type};
use crate::diagnostics::SymbolInfo;
use crate::lexer::{lex, Tok, Token};
use crate::transpiler::rust_name;

pub struct Completion {
    /// The text inserted (`Get`, `trim`, `Push`, a variable name).
    pub label: String,
    /// The teaching line beside it — a VB-facing signature or type.
    pub detail: String,
    pub kind: CompletionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Method,
    Field,
    Variable,
    Function,
    Constant,
    Namespace,
    EnumVariant,
    Enum,
    Struct,
    Keyword,
}

pub fn completions_at(source: &str, offset: usize) -> Vec<Completion> {
    let tokens = lex(source);
    // Inside a string, comment, or verbatim block there is nothing to offer.
    if tokens.iter().any(|t| {
        t.span.start < offset
            && offset < t.span.end
            && matches!(
                t.tok,
                Tok::Str(_)
                    | Tok::Comment(_)
                    | Tok::Backtick(_)
                    | Tok::InlineRust(_)
                    | Tok::InlineCss(_)
                    | Tok::InlinePython { .. }
                    | Tok::TextBlock { .. }
            )
    }) {
        return Vec::new();
    }
    // One compile gives both the program items (functions, enums, structs,
    // constants) and the resolver's typed symbol table. Its diagnostics are
    // irrelevant here — completion runs on broken files constantly.
    let compiled = crate::compile(source);
    let mut diags = crate::diagnostics::Diagnostics::new();
    let program = crate::parser::parse(tokens.clone(), &mut diags);

    match context(source, &tokens, offset) {
        Ctx::Member(receiver) => {
            member_completions(&receiver, offset, &program, &compiled.symbols)
        }
        Ctx::Bare => bare_completions(offset, &tokens, &program, &compiled.symbols),
    }
}

enum Ctx {
    /// The cursor follows `receiver.` (possibly with a partial member typed).
    Member(String),
    Bare,
}

/// What position is the cursor in? Token-level, so it works mid-typing —
/// the broken statement never needs to parse.
fn context(source: &str, tokens: &[Token], offset: usize) -> Ctx {
    let before: Vec<&Token> = tokens
        .iter()
        .filter(|t| t.span.end <= offset && !matches!(t.tok, Tok::Eof | Tok::Comment(_)))
        .collect();
    let mut i = before.len();
    // A word the cursor is still inside is the *filter prefix*, not context —
    // step over it. (A partial word may lex as a keyword: `Do` while typing
    // `Down` — so this checks the source text, not the token kind.)
    if i > 0
        && before[i - 1].span.end == offset
        && source[before[i - 1].span.start..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        i -= 1;
    }
    if i > 0 && matches!(before[i - 1].tok, Tok::Dot) {
        if i > 1 {
            if let Tok::Ident(name) = &before[i - 2].tok {
                return Ctx::Member(name.clone());
            }
        }
        // A dot after something we can't type (a call result, `1.`) — offer
        // nothing rather than something wrong.
        return Ctx::Member(String::new());
    }
    Ctx::Bare
}

// ---- Member completions ---------------------------------------------------------

fn member_completions(
    recv: &str,
    offset: usize,
    program: &Program,
    symbols: &[SymbolInfo],
) -> Vec<Completion> {
    if recv.is_empty() {
        return Vec::new();
    }
    // A typed variable wins (it would shadow a namespace of the same name).
    if let Some(ty) = receiver_type(recv, offset, symbols) {
        return type_members(&ty, program);
    }
    // An enum name → its variants.
    if let Some(e) = program.enums.iter().find(|e| e.name.eq_ignore_ascii_case(recv)) {
        return e
            .variants
            .iter()
            .map(|v| {
                let detail = if v.payload.is_empty() {
                    format!("{}.{}", e.name, v.name)
                } else {
                    let tys: Vec<String> = v.payload.iter().map(|t| t.vb()).collect();
                    format!("{}.{}({})", e.name, v.name, tys.join(", "))
                };
                Completion { label: v.name.clone(), detail, kind: CompletionKind::EnumVariant }
            })
            .collect();
    }
    // `Debug.` — the one built-in pseudo-namespace.
    if recv.eq_ignore_ascii_case("debug") {
        return vec![Completion {
            label: "Print".to_string(),
            detail: "Debug.Print <expr> — print a line to the terminal".to_string(),
            kind: CompletionKind::Method,
        }];
    }
    // A stdlib namespace → its functions.
    if let Some(ns) = crate::transpiler::stdlib_type(recv) {
        return table(namespace_members(ns), CompletionKind::Method);
    }
    Vec::new()
}

/// The declared type of `name` as of `offset` — the nearest recorded
/// occurrence before the cursor (VB scoping is function-level, so the last
/// mention is the right one).
fn receiver_type(name: &str, offset: usize, symbols: &[SymbolInfo]) -> Option<DeclType> {
    let want = rust_name(name);
    symbols
        .iter()
        .filter(|s| s.span.start < offset && rust_name(&s.name) == want)
        .filter_map(|s| s.ty.as_ref().map(|t| (s.span.start, t)))
        .max_by_key(|(start, _)| *start)
        .map(|(_, t)| t.clone())
}

fn type_members(ty: &DeclType, program: &Program) -> Vec<Completion> {
    match ty {
        DeclType::Plain(Type::Text) => table(STRING_METHODS, CompletionKind::Method),
        DeclType::Plain(t) if t.is_float() => table(FLOAT_METHODS, CompletionKind::Method),
        DeclType::Plain(Type::Boolean) => Vec::new(),
        DeclType::Plain(_) => table(INT_METHODS, CompletionKind::Method),
        DeclType::Vec(_) | DeclType::Array(..) | DeclType::Array2D(..) => {
            table(VEC_METHODS, CompletionKind::Method)
        }
        DeclType::Map(..) => table(MAP_METHODS, CompletionKind::Method),
        DeclType::Result(..) => table(RESULT_METHODS, CompletionKind::Method),
        DeclType::Option(..) => table(OPTION_METHODS, CompletionKind::Method),
        DeclType::Tuple(_) => Vec::new(), // elements are .0 / .1
        DeclType::Named(n) => named_members(n, program),
    }
}

/// Members of a named type: a stdlib value (DataFrame, DateTime, Json, …) gets
/// its instance methods; a user struct gets its fields and methods.
fn named_members(name: &str, program: &Program) -> Vec<Completion> {
    let instance = match crate::transpiler::stdlib_type(name) {
        Some("DataFrame") => Some(DATAFRAME_METHODS),
        Some("DateTime") => Some(DATETIME_METHODS),
        Some("Json") => Some(JSON_METHODS),
        Some("Database") => Some(DATABASE_METHODS),
        Some("Process") => Some(PROCESS_METHODS),
        _ => None,
    };
    if let Some(t) = instance {
        return table(t, CompletionKind::Method);
    }
    let mut out = Vec::new();
    if let Some(s) = program.structs.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
        for f in &s.fields {
            out.push(Completion {
                label: f.name.clone(),
                detail: format!("{} As {}", f.name, f.ty.vb()),
                kind: CompletionKind::Field,
            });
        }
    }
    for f in program
        .functions
        .iter()
        .filter(|f| f.receiver.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(name)))
    {
        out.push(Completion {
            label: f.name.clone(),
            detail: fn_signature(f),
            kind: CompletionKind::Method,
        });
    }
    out
}

// ---- Bare completions -----------------------------------------------------------

fn bare_completions(
    offset: usize,
    tokens: &[Token],
    program: &Program,
    symbols: &[SymbolInfo],
) -> Vec<Completion> {
    let mut out = Vec::new();

    // Variables in scope: recorded occurrences before the cursor, inside the
    // enclosing item (the last line-start `Function`/`Sub` before the cursor —
    // VB variables are function-scoped, so nothing from earlier functions).
    let scope_floor = tokens
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            t.span.end <= offset
                && matches!(t.tok, Tok::Function | Tok::Sub)
                && (*i == 0 || matches!(tokens[i - 1].tok, Tok::Newline))
        })
        .map(|(_, t)| t.span.start)
        .last()
        .unwrap_or(0);
    let mut seen: Vec<String> = Vec::new();
    for s in symbols.iter().rev() {
        if s.span.start >= scope_floor && s.span.end <= offset {
            let key = rust_name(&s.name);
            if !seen.contains(&key) {
                seen.push(key);
                out.push(Completion {
                    label: s.name.clone(),
                    detail: s.display.clone(),
                    kind: CompletionKind::Variable,
                });
            }
        }
    }

    for f in program.functions.iter().filter(|f| f.receiver.is_none()) {
        out.push(Completion {
            label: f.name.clone(),
            detail: fn_signature(f),
            kind: CompletionKind::Function,
        });
    }
    for c in &program.constants {
        out.push(Completion {
            label: c.name.clone(),
            detail: format!("Const {} As {}", c.name, c.ty.vb_name()),
            kind: CompletionKind::Constant,
        });
    }
    for e in &program.enums {
        out.push(Completion {
            label: e.name.clone(),
            detail: format!("Enum {}", e.name),
            kind: CompletionKind::Enum,
        });
    }
    for s in &program.structs {
        out.push(Completion {
            label: s.name.clone(),
            detail: format!("Type {}", s.name),
            kind: CompletionKind::Struct,
        });
    }
    for (ns, detail) in NAMESPACES {
        out.push(Completion {
            label: ns.to_string(),
            detail: detail.to_string(),
            kind: CompletionKind::Namespace,
        });
    }
    for kw in KEYWORDS {
        out.push(Completion {
            label: kw.to_string(),
            detail: String::new(),
            kind: CompletionKind::Keyword,
        });
    }
    out
}

/// A function's VB-facing signature, for the detail line.
fn fn_signature(f: &Function) -> String {
    let params: Vec<String> = f.params.iter().map(|p| format!("{} As {}", p.name, p.ty.vb())).collect();
    match &f.ret {
        Some(r) => format!("Function {}({}) As {}", f.name, params.join(", "), r.vb()),
        None => format!("Function {}({})", f.name, params.join(", ")),
    }
}

fn table(rows: &[(&str, &str)], kind: CompletionKind) -> Vec<Completion> {
    rows.iter()
        .map(|(label, detail)| Completion {
            label: label.to_string(),
            detail: detail.to_string(),
            kind,
        })
        .collect()
}

// ---- The catalogues -------------------------------------------------------------
//
// Namespace tables mirror `vbr_stdlib`'s public API (the crate is the ground
// truth — a new stdlib function gets a row here). Value-method tables are the
// curated *real Rust* sets the transpiler already understands (`method_vtype`
// / `is_receiver_typed_method` know their return types) — completion teaches
// the same names the generated Rust uses.

fn namespace_members(ns: &str) -> &'static [(&'static str, &'static str)] {
    match ns {
        "Http" => &[
            ("Get", "Http.Get(url) As Result<String>"),
            ("Post", "Http.Post(url, body, headers) As Result<String>"),
        ],
        "Json" => &[
            ("Parse", "Json.Parse(text) As Result<Json>"),
            ("Object", "Json.Object() As Json — an empty { }"),
            ("Array", "Json.Array() As Json — an empty [ ]"),
        ],
        "FileSystem" => &[
            ("Read", "FileSystem.Read(path) As Result<String>"),
            ("ReadLines", "FileSystem.ReadLines(path) As Result<Vec<String>>"),
            ("Write", "FileSystem.Write(path, contents) As Result<()>"),
            ("Append", "FileSystem.Append(path, text) As Result<()>"),
            ("Exists", "FileSystem.Exists(path) As Boolean"),
            ("Copy", "FileSystem.Copy(source, destination) As Result<()>"),
            ("MoveFile", "FileSystem.MoveFile(source, destination) As Result<()>"),
            ("Delete", "FileSystem.Delete(path) As Result<()>"),
            ("CreateFolder", "FileSystem.CreateFolder(path) As Result<()>"),
            ("CreateFolderAll", "FileSystem.CreateFolderAll(path) As Result<()>"),
            ("FolderExists", "FileSystem.FolderExists(path) As Boolean"),
            ("DeleteFolder", "FileSystem.DeleteFolder(path) As Result<()>"),
            ("DeleteFolderAll", "FileSystem.DeleteFolderAll(path) As Result<()>"),
        ],
        "DateTime" => &[
            ("Now", "DateTime.Now() As DateTime"),
            ("Parse", "DateTime.Parse(text, pattern) As Result<DateTime>"),
        ],
        "Regex" => &[
            ("IsMatch", "Regex.IsMatch(pattern, text) As Result<Boolean>"),
            ("Find", "Regex.Find(pattern, text) As Result<Option<String>>"),
            ("FindAll", "Regex.FindAll(pattern, text) As Result<Vec<String>>"),
            ("Replace", "Regex.Replace(pattern, text, replacement) As Result<String>"),
            ("ReplaceAll", "Regex.ReplaceAll(pattern, text, replacement) As Result<String>"),
            ("Captures", "Regex.Captures(pattern, text) As Result<Vec<String>>"),
        ],
        "DataFrame" => &[("ReadCsv", "DataFrame.ReadCsv(path) As DataFrame")],
        "Database" => &[("Open", "Database.Open(path) As Result<Database>")],
        "Shell" => &[
            ("Run", "Shell.Run(command) As Result<String> — run to completion, capture output"),
            ("Start", "Shell.Start(command) As Result<Process> — launch without waiting"),
        ],
        _ => &[],
    }
}

const NAMESPACES: &[(&str, &str)] = &[
    ("FileSystem", "files and folders"),
    ("Json", "parse and build JSON"),
    ("DateTime", "dates and times"),
    ("Regex", "regular expressions"),
    ("Http", "web requests"),
    ("DataFrame", "tabular data (polars)"),
    ("Database", "SQLite"),
    ("Shell", "run commands"),
];

const KEYWORDS: &[&str] = &[
    "Dim", "Set", "If", "Then", "ElseIf", "Else", "End", "For", "Each", "In", "To", "Step",
    "Next", "Do", "While", "Until", "Loop", "Match", "Return", "Exit", "Continue", "Function",
    "Sub", "Const", "Type", "Enum", "True", "False", "Not", "And", "Or", "Await", "Log", "Test",
    "Assert",
];

const STRING_METHODS: &[(&str, &str)] = &[
    ("len", "len() — length in bytes (Rust: usize)"),
    ("is_empty", "is_empty() As Boolean"),
    ("trim", "trim() — without leading/trailing whitespace"),
    ("trim_start", "trim_start()"),
    ("trim_end", "trim_end()"),
    ("to_uppercase", "to_uppercase() As String"),
    ("to_lowercase", "to_lowercase() As String"),
    ("replace", "replace(from, to) As String"),
    ("repeat", "repeat(n) As String"),
    ("contains", "contains(text) As Boolean"),
    ("starts_with", "starts_with(text) As Boolean"),
    ("ends_with", "ends_with(text) As Boolean"),
    ("to_string", "to_string() — own a borrowed string"),
];

const INT_METHODS: &[(&str, &str)] = &[
    ("abs", "abs() — absolute value"),
    ("min", "min(other)"),
    ("max", "max(other)"),
    ("pow", "pow(exponent)"),
    ("clamp", "clamp(low, high)"),
    ("rem_euclid", "rem_euclid(n) — remainder, never negative"),
];

const FLOAT_METHODS: &[(&str, &str)] = &[
    ("abs", "abs()"),
    ("sqrt", "sqrt()"),
    ("floor", "floor()"),
    ("ceil", "ceil()"),
    ("round", "round()"),
    ("trunc", "trunc() — drop the fraction"),
    ("fract", "fract() — just the fraction"),
    ("powi", "powi(n) — integer exponent"),
    ("powf", "powf(x) — float exponent"),
    ("min", "min(other)"),
    ("max", "max(other)"),
    ("clamp", "clamp(low, high)"),
    ("sin", "sin()"),
    ("cos", "cos()"),
    ("tan", "tan()"),
    ("ln", "ln() — natural log"),
    ("log10", "log10()"),
    ("log2", "log2()"),
    ("exp", "exp()"),
];

const VEC_METHODS: &[(&str, &str)] = &[
    ("Push", "Push(item) — add to the end (Rust: push)"),
    ("Pop", "Pop() As Option<T> — take from the end"),
    ("len", "len() — element count (Rust: usize)"),
    ("is_empty", "is_empty() As Boolean"),
    ("contains", "contains(item) As Boolean"),
    ("sort", "sort() — in place"),
    ("reverse", "reverse() — in place"),
    ("clear", "clear() — remove everything"),
    ("remove", "remove(index) — take one out"),
    ("extend", "extend(other) — append another collection"),
    ("filter", "filter(|x| condition) — keep matching elements"),
    ("map", "map(|x| expression) — transform each element"),
    ("any", "any(|x| condition) As Boolean"),
    ("all", "all(|x| condition) As Boolean"),
    ("find", "find(|x| condition) As Option<T>"),
    ("position", "position(|x| condition) As Option — index of the first match"),
    ("take", "take(n) — the first n"),
    ("skip", "skip(n) — all but the first n"),
    ("join", "join(separator) As String — Vec<String> only"),
];

const MAP_METHODS: &[(&str, &str)] = &[
    ("Insert", "Insert(key, value) (Rust: insert)"),
    ("remove", "remove(key)"),
    ("contains_key", "contains_key(key) As Boolean"),
    ("len", "len() — entry count"),
    ("is_empty", "is_empty() As Boolean"),
    ("clear", "clear()"),
];

const RESULT_METHODS: &[(&str, &str)] = &[
    ("is_ok", "is_ok() As Boolean"),
    ("is_err", "is_err() As Boolean"),
    ("unwrap_or", "unwrap_or(default) — the value, or a fallback"),
];

const OPTION_METHODS: &[(&str, &str)] = &[
    ("is_some", "is_some() As Boolean"),
    ("is_none", "is_none() As Boolean"),
    ("unwrap_or", "unwrap_or(default) — the value, or a fallback"),
];

const DATAFRAME_METHODS: &[(&str, &str)] = &[
    ("Filter", "Filter(formula) As DataFrame — keep matching rows"),
    ("WithColumn", "WithColumn(name, formula) As DataFrame — add/replace a column"),
    ("Select", "Select(columns) As DataFrame"),
    ("Sort", "Sort(column) As DataFrame"),
    ("Head", "Head(n) As DataFrame — the first n rows"),
    ("Shape", "Shape() As (Long, Long) — (rows, columns)"),
    ("Columns", "Columns() As Vec<String>"),
    ("Column", "Column(name) As Vec<T> — one column's values"),
    ("Join", "Join(other, keys) As DataFrame — inner join"),
    ("LeftJoin", "LeftJoin(other, keys) As DataFrame"),
    ("OuterJoin", "OuterJoin(other, keys) As DataFrame"),
    ("GroupBy", "GroupBy(keys) — then .Agg(…)"),
    ("Agg", "Agg(aggregations) As DataFrame — after GroupBy"),
    ("Sum", "Sum(column) As Double"),
    ("Mean", "Mean(column) As Double"),
    ("Min", "Min(column) As Double"),
    ("Max", "Max(column) As Double"),
    ("WriteCsv", "WriteCsv(path)"),
    ("Print", "Print() — show the table"),
];

const DATETIME_METHODS: &[(&str, &str)] = &[
    ("Format", "Format(pattern) As String"),
    ("AddDays", "AddDays(n) As DateTime"),
    ("AddHours", "AddHours(n) As DateTime"),
    ("AddMinutes", "AddMinutes(n) As DateTime"),
    ("DiffDays", "DiffDays(other) As Long"),
    ("DiffHours", "DiffHours(other) As Long"),
    ("Year", "Year() As Integer"),
    ("Month", "Month() As Integer"),
    ("Day", "Day() As Integer"),
];

const JSON_METHODS: &[(&str, &str)] = &[
    ("Get", "Get(key) As Result<Json>"),
    ("GetString", "GetString(key) As Result<String>"),
    ("GetInt", "GetInt(key) As Result<Long>"),
    ("GetFloat", "GetFloat(key) As Result<Double>"),
    ("GetBool", "GetBool(key) As Result<Boolean>"),
    ("GetArray", "GetArray(key) As Result<Vec<Json>>"),
    ("HasKey", "HasKey(key) As Boolean"),
    ("Set", "Set(key, value)"),
    ("SetString", "SetString(key, value)"),
    ("SetInt", "SetInt(key, value)"),
    ("SetBool", "SetBool(key, value)"),
    ("Push", "Push(value) — append to a JSON array"),
    ("AsString", "AsString() As Result<String>"),
    ("AsInt", "AsInt() As Result<Long>"),
    ("AsFloat", "AsFloat() As Result<Double>"),
    ("AsBool", "AsBool() As Result<Boolean>"),
    ("IsNull", "IsNull() As Boolean"),
    ("ToString", "ToString() As Result<String> — compact JSON text"),
    ("ToPretty", "ToPretty() As Result<String> — indented JSON text"),
];

const DATABASE_METHODS: &[(&str, &str)] = &[
    ("Execute", "Execute(sql, params) As Result<Long> — rows affected"),
    ("Query", "Query(sql, params) As Result<Vec<Json>> — one Json per row"),
    ("LastInsertId", "LastInsertId() As Long"),
];

const PROCESS_METHODS: &[(&str, &str)] = &[
    ("IsRunning", "IsRunning() As Boolean"),
    ("Wait", "Wait() As Long — block until it exits, return the exit code"),
    ("Kill", "Kill() — stop the process"),
];
