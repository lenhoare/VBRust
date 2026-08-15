//! `vbr help build` — the offline help system.
//!
//! Ethos: fast, offline, example-first — the VBA / Turbo Pascal help that made
//! those tools productive. The **driver** is the hand-authored TOML in
//! `help/entries/`; the **output** (`help/build/`) is a human convenience, fully
//! regenerated each run and never read back in.
//!
//! Two guarantees keep the help honest, both borrowed from the snapshot tests:
//!   * every keyword/builtin the compiler knows is listed in [`help_manifest`],
//!     so `coverage.md` can flag any topic that lacks an entry; and
//!   * every entry's example is compiled through [`crate::compile`], so a broken
//!     example fails the build — the code you read always transpiles.
//!
//! Output skins (both from the same entries):
//!   * `build/site/`  — an offline HTML/JS site (open `index.html`); search is
//!     client-side over a generated `data.js` (no server, works from `file://`).
//!   * `build/text/`  — one Markdown file per entry, for review and for the TUI
//!     IDE to show inline (the second skin).
//!
//! Anchor scheme (stable, for a future F1 hook): `#kw/For`, `#fn/Mid`,
//! `#op/Is`, `#ty/Long`. Keyword-under-cursor → deep link.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde_json::json;

/// The four families of help topic. The first letter drives the anchor prefix.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Keyword,
    Builtin,
    Type,
    Operator,
    Namespace,
}

/// Anchor prefix for a `kind` string (as authored in an entry's `kind =` line).
fn anchor_prefix(kind: &str) -> &'static str {
    match kind {
        "function" | "builtin" => "fn",
        "type" => "ty",
        "operator" => "op",
        "namespace" => "ns",
        _ => "kw",
    }
}

/// One topic the compiler knows exists. This is the completeness backstop: the
/// list is derived from the same vocabulary the lexer and resolver accept, so a
/// keyword can never silently go undocumented.
pub struct ManifestItem {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub kind: Kind,
    pub anchor: &'static str,
}

macro_rules! item {
    ($id:literal, $title:literal, $cat:literal, $kind:expr, $anchor:literal) => {
        ManifestItem { id: $id, title: $title, category: $cat, kind: $kind, anchor: $anchor }
    };
}

/// Every core-language topic, in sidebar / coverage order. Slice 1 authors a
/// handful of these; the rest surface in `coverage.md` as stubs to be written.
pub fn help_manifest() -> Vec<ManifestItem> {
    use Kind::*;
    vec![
        // Declarations
        item!("dim", "Dim", "Declarations", Keyword, "kw/Dim"),
        item!("const", "Const", "Declarations", Keyword, "kw/Const"),
        item!("redim", "ReDim", "Declarations", Keyword, "kw/ReDim"),
        item!("set", "Set", "Declarations", Keyword, "kw/Set"),
        item!("public", "Public", "Declarations", Keyword, "kw/Public"),
        item!("private", "Private", "Declarations", Keyword, "kw/Private"),
        // Procedures
        item!("function", "Function", "Procedures", Keyword, "kw/Function"),
        item!("sub", "Sub", "Procedures", Keyword, "kw/Sub"),
        item!("return", "Return", "Procedures", Keyword, "kw/Return"),
        item!("raiseerror", "RaiseError", "Procedures", Keyword, "kw/RaiseError"),
        item!("handle", "Handle", "Procedures", Keyword, "kw/Handle"),
        item!("raw", "Raw", "Procedures", Keyword, "kw/Raw"),
        // Control flow
        item!("if", "If…Then…Else", "Control flow", Keyword, "kw/If"),
        item!("match", "Match", "Control flow", Keyword, "kw/Match"),
        item!("for", "For…Next", "Control flow", Keyword, "kw/For"),
        item!("for-each", "For Each", "Control flow", Keyword, "kw/ForEach"),
        item!("do-loop", "Do…Loop", "Control flow", Keyword, "kw/Do"),
        item!("while", "While", "Control flow", Keyword, "kw/While"),
        item!("exit", "Exit", "Control flow", Keyword, "kw/Exit"),
        item!("continue", "Continue", "Control flow", Keyword, "kw/Continue"),
        item!("with", "With", "Control flow", Keyword, "kw/With"),
        item!("await", "Await", "Control flow", Keyword, "kw/Await"),
        item!("theme", "Theme", "Surfaces", Keyword, "kw/Theme"),
        // Escape hatches
        item!("rust", "Rust … End Rust", "Escape hatches", Keyword, "kw/Rust"),
        item!("python", "Python … End Python", "Escape hatches", Keyword, "kw/Python"),
        // Custom types
        item!("type", "Type", "Custom types", Keyword, "kw/Type"),
        item!("enum", "Enum", "Custom types", Keyword, "kw/Enum"),
        item!("new", "New", "Custom types", Keyword, "kw/New"),
        // Collections & wrappers (first-class generic types)
        item!("vec", "Vec<T>", "Collections & wrappers", Type, "ty/Vec"),
        item!("array", "Arrays", "Collections & wrappers", Type, "ty/Array"),
        item!("hashmap", "HashMap<K,V>", "Collections & wrappers", Type, "ty/HashMap"),
        item!("option", "Option<T>", "Collections & wrappers", Type, "ty/Option"),
        item!("result", "Result<T>", "Collections & wrappers", Type, "ty/Result"),
        item!("question", "? (try)", "Collections & wrappers", Operator, "op/Question"),
        // Operators
        item!("and", "And", "Operators", Operator, "op/And"),
        item!("or", "Or", "Operators", Operator, "op/Or"),
        item!("not", "Not", "Operators", Operator, "op/Not"),
        item!("xor", "Xor", "Operators", Operator, "op/Xor"),
        item!("mod", "Mod", "Operators", Operator, "op/Mod"),
        item!("is", "Is", "Operators", Operator, "op/Is"),
        // Data types
        item!("integer", "Integer", "Data types", Type, "ty/Integer"),
        item!("long", "Long", "Data types", Type, "ty/Long"),
        item!("longlong", "LongLong", "Data types", Type, "ty/LongLong"),
        item!("single", "Single", "Data types", Type, "ty/Single"),
        item!("double", "Double", "Data types", Type, "ty/Double"),
        item!("boolean", "Boolean", "Data types", Type, "ty/Boolean"),
        item!("byte", "Byte", "Data types", Type, "ty/Byte"),
        item!("string", "String", "Data types", Type, "ty/String"),
        item!("currency", "Currency", "Data types", Type, "ty/Currency"),
        item!("variant", "Variant", "Data types", Type, "ty/Variant"),
        // String functions
        item!("len", "Len", "String functions", Builtin, "fn/Len"),
        item!("left", "Left", "String functions", Builtin, "fn/Left"),
        item!("right", "Right", "String functions", Builtin, "fn/Right"),
        item!("mid", "Mid", "String functions", Builtin, "fn/Mid"),
        item!("trim", "Trim", "String functions", Builtin, "fn/Trim"),
        item!("ucase", "UCase", "String functions", Builtin, "fn/UCase"),
        item!("lcase", "LCase", "String functions", Builtin, "fn/LCase"),
        item!("replace", "Replace", "String functions", Builtin, "fn/Replace"),
        item!("instr", "InStr", "String functions", Builtin, "fn/InStr"),
        item!("asc", "Asc", "String functions", Builtin, "fn/Asc"),
        item!("chr", "Chr", "String functions", Builtin, "fn/Chr"),
        // Conversion
        item!("cstr", "CStr", "Conversion", Builtin, "fn/CStr"),
        item!("str", "Str", "Conversion", Builtin, "fn/Str"),
        item!("val", "Val", "Conversion", Builtin, "fn/Val"),
        item!("iif", "IIf", "Conversion", Builtin, "fn/IIf"),
        // Math functions
        item!("abs", "Abs", "Math functions", Builtin, "fn/Abs"),
        item!("int", "Int", "Math functions", Builtin, "fn/Int"),
        item!("round", "Round", "Math functions", Builtin, "fn/Round"),
        item!("sqr", "Sqr", "Math functions", Builtin, "fn/Sqr"),
        item!("sin", "Sin", "Math functions", Builtin, "fn/Sin"),
        item!("cos", "Cos", "Math functions", Builtin, "fn/Cos"),
        item!("tan", "Tan", "Math functions", Builtin, "fn/Tan"),
        item!("log", "Log", "Math functions", Builtin, "fn/Log"),
        item!("exp", "Exp", "Math functions", Builtin, "fn/Exp"),
        // Standard library (namespaces)
        item!("filesystem", "FileSystem", "Standard library", Namespace, "ns/FileSystem"),
        item!("http", "Http", "Standard library", Namespace, "ns/Http"),
        item!("database", "Database", "Standard library", Namespace, "ns/Database"),
        item!("json", "Json", "Standard library", Namespace, "ns/Json"),
        item!("datetime", "DateTime", "Standard library", Namespace, "ns/DateTime"),
        item!("regex", "Regex", "Standard library", Namespace, "ns/Regex"),
        item!("dataframe", "DataFrame", "Standard library", Namespace, "ns/DataFrame"),
        item!("shell", "Shell", "Standard library", Namespace, "ns/Shell"),
    ]
}

// ---------------------------------------------------------------------------
// Entry file (`help/entries/<id>.toml`)
// ---------------------------------------------------------------------------

/// One authored help topic. Authored as TOML; we parse the small subset we use
/// (single-line strings, `"""` multi-line blocks, and string arrays) rather
/// than pulling in a TOML crate — the format stays valid TOML regardless.
#[derive(Default)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub category: String,
    pub kind: String,
    /// Set on a member page (`vec.push`); names the owning type's id (`vec`).
    /// Member pages are reachable from their parent and by search, but are kept
    /// out of the main sidebar so it stays a clean table of contents.
    pub parent: String,
    /// Optional explicit anchor override (member pages use `m/Vec.Push`).
    pub anchor: String,
    pub syntax: String,
    pub replaces: String,
    pub summary: String,
    pub arguments: Vec<Arg>,
    pub returns: String,
    pub remarks: String,
    pub members: Vec<Member>,
    pub example: String,
    pub see_also: Vec<String>,
    /// Keys into the shared [`crate::diagnostics::CAUTIONS`] registry. Each
    /// renders a "Caution" callout below Syntax, using the *same* text the
    /// compiler shows as a teaching note — one source, so they never drift.
    pub caution: Vec<String>,
}

/// One parameter of a member, authored `name | type | description`.
pub struct Arg {
    pub name: String,
    pub ty: String,
    pub desc: String,
}

fn parse_args(raw: &str) -> Vec<Arg> {
    raw.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() {
                return None;
            }
            let parts: Vec<&str> = l.splitn(3, '|').collect();
            if parts.len() < 2 {
                return None;
            }
            Some(Arg {
                name: parts[0].trim().to_string(),
                ty: parts[1].trim().to_string(),
                desc: parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
            })
        })
        .collect()
}

/// A property or method (action) on a type — VB-style. Authored one per line in
/// a `members = """…"""` block as `kind | signature | description`.
pub struct Member {
    pub kind: String, // "property" | "method"
    pub sig: String,
    pub desc: String,
}

fn parse_members(raw: &str) -> Vec<Member> {
    raw.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() {
                return None;
            }
            let parts: Vec<&str> = l.splitn(3, '|').collect();
            if parts.len() < 2 {
                return None;
            }
            Some(Member {
                kind: parts[0].trim().to_lowercase(),
                sig: parts[1].trim().to_string(),
                desc: parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
            })
        })
        .collect()
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').unwrap_or(s);
    let s = s.strip_suffix('"').unwrap_or(s);
    s.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// Parse the TOML subset used by entry files into `key -> value` maps.
fn parse_toml_ish(
    src: &str,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, Vec<String>>), String> {
    let mut strings = BTreeMap::new();
    let mut arrays = BTreeMap::new();
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            i += 1;
            continue;
        }
        let eq = line
            .find('=')
            .ok_or_else(|| format!("line {}: expected `key = value`", i + 1))?;
        let key = line[..eq].trim().to_string();
        let val = line[eq + 1..].trim();
        if val.starts_with("\"\"\"") {
            // Multi-line block: collect verbatim until a line that is just `"""`.
            let mut buf: Vec<&str> = Vec::new();
            i += 1;
            while i < lines.len() && lines[i].trim() != "\"\"\"" {
                buf.push(lines[i]);
                i += 1;
            }
            strings.insert(key, buf.join("\n"));
            i += 1; // consume the closing """
        } else if val.starts_with('[') {
            let inner = val.trim_start_matches('[').trim_end_matches(']');
            let arr: Vec<String> = inner
                .split(',')
                .map(|s| unquote(s))
                .filter(|s| !s.is_empty())
                .collect();
            arrays.insert(key, arr);
            i += 1;
        } else {
            strings.insert(key, unquote(val));
            i += 1;
        }
    }
    Ok((strings, arrays))
}

fn parse_entry(src: &str) -> Result<Entry, String> {
    let (s, a) = parse_toml_ish(src)?;
    let get = |k: &str| s.get(k).cloned().unwrap_or_default();
    let id = get("id");
    if id.is_empty() {
        return Err("missing `id`".into());
    }
    Ok(Entry {
        id,
        title: get("title"),
        category: get("category"),
        kind: {
            let k = get("kind");
            if k.is_empty() {
                "keyword".into()
            } else {
                k
            }
        },
        parent: get("parent"),
        anchor: get("anchor"),
        syntax: get("syntax"),
        replaces: get("replaces"),
        summary: get("summary"),
        arguments: parse_args(&get("arguments")),
        returns: get("returns"),
        remarks: get("remarks"),
        members: parse_members(&get("members")),
        example: get("example"),
        see_also: a.get("see_also").cloned().unwrap_or_default(),
        caution: a.get("caution").cloned().unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

/// What a build produced, for the CLI to report.
pub struct Report {
    pub written: usize,
    pub members: usize,
    pub covered: usize,
    pub total: usize,
    pub stubs: Vec<String>,
    pub failures: Vec<(String, String)>,
}

/// Read `entries_dir/*.toml`, verify + render, and write the two skins plus the
/// coverage report into `out_dir`. Returns a [`Report`]; the caller decides how
/// loudly to fail on `failures`.
pub fn build(entries_dir: &Path, out_dir: &Path) -> Result<Report, String> {
    // 1. Load and parse every entry.
    let mut entries: Vec<Entry> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let read = fs::read_dir(entries_dir)
        .map_err(|e| format!("cannot read {}: {}", entries_dir.display(), e))?;
    let mut paths: Vec<_> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
        .collect();
    paths.sort();
    for path in &paths {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        match parse_entry(&text) {
            Ok(e) => entries.push(e),
            Err(e) => failures.push((path.display().to_string(), e)),
        }
    }

    // 2. Compile each example; a broken example is a build failure.
    let mut rust_of: BTreeMap<String, String> = BTreeMap::new();
    for e in &entries {
        if e.example.trim().is_empty() {
            continue;
        }
        let compiled = crate::compile(&e.example);
        if compiled.has_errors {
            let first = compiled
                .diagnostics
                .first()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "example did not compile".into());
            failures.push((format!("{} (example)", e.id), first));
        } else {
            rust_of.insert(e.id.clone(), compiled.rust);
        }
    }

    // 2b. The real gate: type-check each generated program with rustc, so an
    // example that transpiles but isn't valid Rust (a mis-spelled method, say)
    // is caught here rather than shipped.
    // Core examples check with bare rustc (fast); stdlib examples need
    // `vbr_stdlib`, so they go through a batched `cargo check`. Inline Python
    // generates pyo3 glue, which needs the project build and a Python install
    // — transpile only, same as a Window that cannot pass bare rustc.
    let (stdlib_ex, rest): (Vec<(String, String)>, Vec<(String, String)>) = rust_of
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .partition(|(_, r)| r.contains("vbr_stdlib"));
    let core_ex: Vec<(String, String)> = rest
        .into_iter()
        .filter(|(_, r)| !r.contains("pyo3::"))
        .collect();
    failures.append(&mut rustc_check_all(&core_ex));
    failures.append(&mut stdlib_check_all(&stdlib_ex));

    // 3. Order entries by manifest position (sidebar order); unknown ids last.
    let manifest = help_manifest();
    let order: BTreeMap<&str, usize> =
        manifest.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
    let anchor_of: BTreeMap<&str, &str> =
        manifest.iter().map(|m| (m.id, m.anchor)).collect();
    entries.sort_by_key(|e| *order.get(e.id.as_str()).unwrap_or(&usize::MAX));

    // 4. Emit.
    let site = out_dir.join("site");
    let text = out_dir.join("text");
    fs::create_dir_all(&site).map_err(|e| e.to_string())?;
    fs::create_dir_all(&text).map_err(|e| e.to_string())?;

    let anchor_for = |e: &Entry| -> String {
        if !e.anchor.is_empty() {
            return e.anchor.clone();
        }
        anchor_of
            .get(e.id.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/{}", anchor_prefix(&e.kind), e.id))
    };

    // data.js — all entries + generated Rust, as a JS global (no fetch needed).
    let mut arr = Vec::new();
    for e in &entries {
        let rust = rust_of.get(&e.id).cloned().unwrap_or_default();
        // Resolve caution keys against the shared registry; a typo is a build
        // error, so a doc page can't silently reference a note that isn't there.
        let cautions = e
            .caution
            .iter()
            .map(|k| {
                crate::diagnostics::caution(k)
                    .map(|c| json!({ "summary": c.summary, "body": c.body }))
                    .ok_or_else(|| format!("entry `{}`: unknown caution key `{k}`", e.id))
            })
            .collect::<Result<Vec<_>, String>>()?;
        arr.push(json!({
            "id": e.id,
            "title": e.title,
            "category": e.category,
            "kind": e.kind,
            "anchor": anchor_for(e),
            "parent": e.parent,
            "summary": e.summary,
            "replaces": e.replaces,
            "cautions": cautions,
            "arguments": e.arguments.iter().map(|a| json!({ "name": a.name, "ty": a.ty, "desc": a.desc })).collect::<Vec<_>>(),
            "returns": e.returns,
            "remarks": e.remarks,
            "properties": members_json(&e.members, "property"),
            "methods": members_json(&e.members, "method"),
            "see_also": e.see_also,
            // Pre-highlighted HTML (built here so the page stays static). The
            // copy buttons read the rendered text, so no plain copy is needed.
            "syntax_html": highlight_vbr(&e.syntax),
            "example_html": highlight_vbr(&e.example),
            "rust_html": highlight_rust(&rust),
            "has_syntax": !e.syntax.trim().is_empty(),
            "has_example": !e.example.trim().is_empty(),
            "has_rust": !rust.trim().is_empty(),
        }));
    }
    // Every known topic's display title + anchor, so "See also" can name a
    // topic properly (capitalised) even before it has been authored.
    let mut topics = serde_json::Map::new();
    for m in &manifest {
        topics.insert(m.id.to_string(), json!({ "title": m.title, "anchor": m.anchor }));
    }
    let data = json!({ "entries": arr, "topics": topics });
    let data_js = format!(
        "// GENERATED by `vbr help build` — do not edit. Regenerated from help/entries/.\nwindow.VBR_HELP = {};\n",
        serde_json::to_string_pretty(&data).unwrap()
    );
    fs::write(site.join("data.js"), &data_js).map_err(|e| e.to_string())?;
    fs::write(site.join("app.js"), APP_JS).map_err(|e| e.to_string())?;
    // Stamp the script URLs with a content hash so browsers (phones, especially)
    // can't serve a stale app.js/data.js after a rebuild — the URL changes.
    let ver = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        APP_JS.hash(&mut h);
        data_js.hash(&mut h);
        format!("{:x}", h.finish())
    };
    let index_html = INDEX_HTML
        .replace("src=\"data.js\"", &format!("src=\"data.js?v={ver}\""))
        .replace("src=\"app.js\"", &format!("src=\"app.js?v={ver}\""));
    fs::write(site.join("index.html"), index_html).map_err(|e| e.to_string())?;

    // text/<id>.md — the second skin.
    for e in &entries {
        let md = render_markdown(e, &anchor_for(e), rust_of.get(&e.id).map(|s| s.as_str()));
        fs::write(text.join(format!("{}.md", e.id)), md).map_err(|e| e.to_string())?;
    }

    // coverage.md — manifest vs authored.
    let covered_ids: std::collections::HashSet<&str> =
        entries.iter().map(|e| e.id.as_str()).collect();
    let mut stubs = Vec::new();
    let coverage = render_coverage(&manifest, &covered_ids, &mut stubs);
    fs::write(out_dir.join("coverage.md"), coverage).map_err(|e| e.to_string())?;

    failures.sort();
    let members = entries.iter().filter(|e| !e.parent.is_empty()).count();
    Ok(Report {
        written: entries.len(),
        members,
        covered: manifest.iter().filter(|m| covered_ids.contains(m.id)).count(),
        total: manifest.len(),
        stubs,
        failures,
    })
}

// ---------------------------------------------------------------------------
// The real gate: every example must compile with rustc, not merely transpile.
// (Transpile-only let `.IsEmpty` → the invalid `isempty` slip through.) The
// examples are single-file, no-stdlib programs, so a bare `rustc --emit=metadata`
// type-checks them. Runs in parallel; skips cleanly if rustc isn't installed.
// ---------------------------------------------------------------------------

fn rustc_available() -> bool {
    Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn rustc_check(id: &str, rust: &str, dir: &Path) -> Result<(), String> {
    let stem = id.replace('.', "_");
    let src = dir.join(format!("{stem}.rs"));
    let meta = dir.join(format!("{stem}.rmeta"));
    if let Err(e) = fs::write(&src, rust) {
        return Err(format!("could not write temp file: {e}"));
    }
    let out = Command::new("rustc")
        .args(["--edition", "2021", "--emit=metadata", "-A", "warnings", "--crate-type", "bin", "-o"])
        .arg(&meta)
        .arg(&src)
        .output();
    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&meta);
    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stderr);
            let msg = s
                .lines()
                .find(|l| l.trim_start().starts_with("error"))
                .unwrap_or("rustc rejected the example")
                .trim()
                .to_string();
            Err(msg)
        }
        Err(e) => Err(format!("could not run rustc: {e}")),
    }
}

/// Type-check every example with rustc, in parallel. Returns `(id, error)` for
/// each that fails. If rustc is missing, returns empty and warns once.
fn rustc_check_all(examples: &[(String, String)]) -> Vec<(String, String)> {
    if !rustc_available() {
        eprintln!("⚠ rustc not found — skipping the compile check (examples were transpiled only).");
        return Vec::new();
    }
    let dir = std::env::temp_dir().join("vbr_help_check");
    let _ = fs::create_dir_all(&dir);
    let failures = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= examples.len() {
                    break;
                }
                let (id, rust) = &examples[i];
                if let Err(e) = rustc_check(id, rust, &dir) {
                    failures.lock().unwrap().push((format!("{id} (rustc)"), e));
                }
            });
        }
    });
    failures.into_inner().unwrap()
}

// ---------------------------------------------------------------------------
// Stdlib examples pull in `vbr_stdlib`, so bare rustc can't check them. Compile
// them together in one throwaway cargo project (heavy deps like polars build
// once and cache) with `cargo check` — type-checks, never runs, so no I/O.
// ---------------------------------------------------------------------------

/// Namespaces that need a `vbr_stdlib` Cargo feature; FileSystem/Shell are std-only.
const STDLIB_FEATURES: &[(&str, &str)] = &[
    ("Json", "json"),
    ("DateTime", "datetime"),
    ("Regex", "regex"),
    ("Http", "http"),
    ("DataFrame", "dataframe"),
    ("Database", "database"),
];

fn stdlib_crate_path() -> String {
    std::env::var("VBR_STDLIB_PATH")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/vbr_stdlib").to_string())
}

fn cargo_available() -> bool {
    Command::new("cargo").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn stdlib_check_all(examples: &[(String, String)]) -> Vec<(String, String)> {
    if examples.is_empty() {
        return Vec::new();
    }
    if !cargo_available() {
        eprintln!("⚠ cargo not found — skipping the stdlib compile check.");
        return Vec::new();
    }
    // Only the features the examples actually use, so a FileSystem-only run
    // never has to build polars.
    let mut feats: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for (_, rust) in examples {
        for (ns, feat) in STDLIB_FEATURES {
            if rust.contains(ns) {
                feats.insert(feat);
            }
        }
    }
    let feat_list = feats.iter().map(|f| format!("{f:?}")).collect::<Vec<_>>().join(", ");
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target/help_stdlib_check"));
    let bin = dir.join("src/bin");
    if fs::create_dir_all(&bin).is_err() {
        eprintln!("⚠ could not create the stdlib check project — skipping.");
        return Vec::new();
    }
    // A private `[workspace]` keeps this from joining the vbr workspace.
    let cargo_toml = format!(
        "[package]\nname = \"help_stdlib_check\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [workspace]\n\n[dependencies]\nvbr_stdlib = {{ path = {:?}, default-features = false, features = [{}] }}\n",
        stdlib_crate_path(),
        feat_list
    );
    let _ = fs::write(dir.join("Cargo.toml"), &cargo_toml);
    let _ = fs::remove_dir_all(&bin);
    let _ = fs::create_dir_all(&bin);
    let mut id_of_bin: BTreeMap<String, String> = BTreeMap::new();
    for (id, rust) in examples {
        let stem = id.replace('.', "_");
        let _ = fs::write(bin.join(format!("{stem}.rs")), rust);
        id_of_bin.insert(stem, id.clone());
    }
    let out = match Command::new("cargo")
        .args(["check", "--quiet", "--message-format=json"])
        .current_dir(&dir)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("⚠ cargo check failed to run: {e}");
            return Vec::new();
        }
    };
    let mut failures = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["reason"] != "compiler-message" || v["message"]["level"] != "error" {
            continue;
        }
        if let Some(id) = id_of_bin.get(v["target"]["name"].as_str().unwrap_or("")) {
            if seen.insert(id.clone()) {
                let msg = v["message"]["message"].as_str().unwrap_or("did not compile");
                failures.push((format!("{id} (cargo)"), msg.to_string()));
            }
        }
    }
    // A dependency/build failure not tied to one bin.
    if !out.status.success() && failures.is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.lines().find(|l| l.contains("error")).unwrap_or("cargo check failed");
        failures.push(("stdlib (cargo)".to_string(), msg.trim().to_string()));
    }
    failures
}

fn members_json(members: &[Member], kind: &str) -> serde_json::Value {
    serde_json::Value::Array(
        members
            .iter()
            .filter(|m| m.kind == kind)
            .map(|m| json!({ "sig": m.sig, "desc": m.desc }))
            .collect(),
    )
}

fn render_markdown(e: &Entry, anchor: &str, rust: Option<&str>) -> String {
    let mut m = String::new();
    m.push_str("<!-- GENERATED by `vbr help build` — do not edit. -->\n\n");
    m.push_str(&format!("# {}  `{}`\n\n", e.title, e.kind));
    if e.parent.is_empty() {
        m.push_str(&format!("`#{}` · {}\n\n", anchor, e.category));
    } else {
        m.push_str(&format!("`#{}` · a member of {}\n\n", anchor, e.category));
    }
    if !e.summary.is_empty() {
        m.push_str(&format!("{}\n\n", e.summary));
    }
    if !e.syntax.is_empty() {
        m.push_str(&format!("## Syntax\n\n```vb\n{}\n```\n\n", e.syntax));
    }
    if !e.replaces.is_empty() {
        m.push_str(&format!("> ⇄ **Replaces** VB's {}\n\n", e.replaces));
    }
    if !e.arguments.is_empty() {
        m.push_str("## Arguments\n\n");
        for a in &e.arguments {
            m.push_str(&format!("- `{}` (`{}`) — {}\n", a.name, a.ty, a.desc));
        }
        m.push('\n');
    }
    if !e.returns.is_empty() {
        m.push_str(&format!("## Returns\n\n{}\n\n", e.returns));
    }
    if !e.remarks.is_empty() {
        m.push_str(&format!("## Remarks\n\n{}\n\n", e.remarks));
    }
    for (heading, kind) in [("Properties", "property"), ("Methods", "method")] {
        let list: Vec<&Member> = e.members.iter().filter(|x| x.kind == kind).collect();
        if list.is_empty() {
            continue;
        }
        m.push_str(&format!("## {}\n\n", heading));
        for x in list {
            if x.desc.is_empty() {
                m.push_str(&format!("- `{}`\n", x.sig));
            } else {
                m.push_str(&format!("- `{}` — {}\n", x.sig, x.desc));
            }
        }
        m.push('\n');
    }
    if !e.example.is_empty() {
        m.push_str(&format!("## Example\n\n```vb\n{}\n```\n\n", e.example));
    }
    if let Some(r) = rust {
        if !r.trim().is_empty() {
            m.push_str(&format!("## Generated Rust\n\n```rust\n{}\n```\n\n", r.trim_end()));
        }
    }
    if !e.see_also.is_empty() {
        m.push_str("## See also\n\n");
        for s in &e.see_also {
            m.push_str(&format!("- `{}`\n", s));
        }
        m.push('\n');
    }
    m
}

fn render_coverage(
    manifest: &[ManifestItem],
    covered: &std::collections::HashSet<&str>,
    stubs: &mut Vec<String>,
) -> String {
    let mut m = String::new();
    m.push_str("<!-- GENERATED by `vbr help build` — do not edit. -->\n\n");
    m.push_str("# Help coverage\n\n");
    m.push_str("Derived from the compiler's own keyword/builtin manifest, so nothing can go silently undocumented. ✓ = an entry exists in `help/entries/`.\n\n");
    let mut current = "";
    for item in manifest {
        if item.category != current {
            m.push_str(&format!("\n## {}\n\n", item.category));
            current = item.category;
        }
        let has = covered.contains(item.id);
        if !has {
            stubs.push(item.id.to_string());
        }
        m.push_str(&format!(
            "- {} {} — `#{}`{}\n",
            if has { "✓" } else { "☐" },
            item.title,
            item.anchor,
            if has { "" } else { "  _(stub)_" }
        ));
    }
    let done = manifest.iter().filter(|i| covered.contains(i.id)).count();
    m.push_str(&format!(
        "\n---\n\n**{} of {} topics documented.**\n",
        done,
        manifest.len()
    ));
    m
}

const INDEX_HTML: &str = include_str!("help_assets/index.html");
const APP_JS: &str = include_str!("help_assets/app.js");

// ---------------------------------------------------------------------------
// Syntax highlighting — done at build time, so the site stays static.
//
// Bust is highlighted by the *compiler's own lexer*, so the colours can never
// drift from the language. The generated Rust gets a small dedicated scanner.
// ---------------------------------------------------------------------------

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// CSS class for a Bust token, or `None` to emit it uncoloured.
fn vbr_class(t: &crate::lexer::Tok) -> Option<&'static str> {
    use crate::lexer::Tok::*;
    Some(match t {
        Function | Sub | Return | ByVal | ByRef | End | Dim | Set | Nothing | Mut | As | If
        | Then | ElseIf | Else | Select | Case | Match | Await | For | Each | In | To | Step
        | Next | New | Do | Loop | While | Until | Exit | Continue | With | And | Or | Not
        | Xor | Mod | True | False | Type | Enum | Public | Private | Const | ReDim | On => "t-kw",
        TyInteger | TyLong | TyLongLong | TySingle | TyDouble | TyBoolean | TyByte | TyString
        | TyCurrency | TyVariant => "t-type",
        Int(_) | Float(_) => "t-num",
        Str(_) | Backtick(_) | TextBlock { .. } => "t-str",
        Comment(_) => "t-comment",
        _ => return None,
    })
}

/// Highlight Bust by walking the real token stream and wrapping each token in a
/// span, preserving the exact source text (whitespace, comments, layout).
fn highlight_vbr(src: &str) -> String {
    let toks = crate::lexer::lex(src);
    let mut out = String::new();
    let mut pos = 0usize;
    for t in &toks {
        if matches!(t.tok, crate::lexer::Tok::Eof) {
            continue;
        }
        let (s, e) = (t.span.start, t.span.end);
        if s < pos || e > src.len() || s > e {
            continue; // be defensive; never panic in a doc build
        }
        out.push_str(&esc_html(&src[pos..s]));
        let text = esc_html(&src[s..e]);
        match vbr_class(&t.tok) {
            Some(c) => {
                out.push_str("<span class=\"");
                out.push_str(c);
                out.push_str("\">");
                out.push_str(&text);
                out.push_str("</span>");
            }
            None => out.push_str(&text),
        }
        pos = e;
    }
    if pos < src.len() {
        out.push_str(&esc_html(&src[pos..]));
    }
    out
}

/// A small scanner for the generated Rust: keywords, primitive/`Uppercase`
/// types, strings, line comments, numbers.
fn highlight_rust(src: &str) -> String {
    const KW: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
        "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type",
        "unsafe", "use", "where", "while",
    ];
    const PRIM: &[&str] = &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str",
    ];
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0;
    let slice = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    while i < n {
        let c = chars[i];
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            let start = i;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            out.push_str("<span class=\"t-comment\">");
            out.push_str(&esc_html(&slice(start, i)));
            out.push_str("</span>");
        } else if c == '"' {
            let start = i;
            i += 1;
            while i < n {
                if chars[i] == '\\' && i + 1 < n {
                    i += 2;
                    continue;
                }
                if chars[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str("<span class=\"t-str\">");
            out.push_str(&esc_html(&slice(start, i)));
            out.push_str("</span>");
        } else if c.is_ascii_digit() {
            let start = i;
            while i < n && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            out.push_str("<span class=\"t-num\">");
            out.push_str(&esc_html(&slice(start, i)));
            out.push_str("</span>");
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word = slice(start, i);
            let cls = if KW.contains(&word.as_str()) {
                Some("t-kw")
            } else if PRIM.contains(&word.as_str())
                || word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            {
                Some("t-type")
            } else {
                None
            };
            match cls {
                Some(c) => {
                    out.push_str("<span class=\"");
                    out.push_str(c);
                    out.push_str("\">");
                    out.push_str(&esc_html(&word));
                    out.push_str("</span>");
                }
                None => out.push_str(&esc_html(&word)),
            }
        } else {
            out.push_str(&esc_html(&c.to_string()));
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_ids_are_unique() {
        let m = help_manifest();
        let mut seen = std::collections::HashSet::new();
        for i in &m {
            assert!(seen.insert(i.id), "duplicate manifest id {}", i.id);
        }
    }

    #[test]
    fn parses_a_multiline_entry() {
        let src = r#"
id = "for"
title = "For"
category = "Control flow"
kind = "keyword"
summary = "Counts a variable through a range."
example = """
Function Main()
    Dim n As Long = 0
End Function
"""
see_also = ["do-loop", "while"]
"#;
        let e = parse_entry(src).unwrap();
        assert_eq!(e.id, "for");
        assert_eq!(e.kind, "keyword");
        assert!(e.example.contains("Dim n As Long"));
        assert_eq!(e.see_also, vec!["do-loop", "while"]);
    }
}
