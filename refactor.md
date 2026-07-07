# Refactorings — session of 2026-07-06/07

What changed *structurally* this session (the features these enabled: Page
async + styling, the browser Screen via Ratzilla, the playground). Each kept
existing output byte-identical unless noted.

## 1. `AsyncBackend` — one `Await` machine, three shells

`surface.rs` gained `enum AsyncBackend { Native, Web, WebScreen }`, threaded
through `analyze_events` → `await_split` → `awaitable_info` → `snapshot_args`.
It decides:

- the **state receiver** snapshots clone from (`state.url.clone()` for
  Native/WebScreen, `self.url.clone()` for a Yew Page);
- the **call form** an awaited `Http.Get` lowers to (`Http::get` +
  `spawn_blocking`/thread natively; the generated `http_get` fetch wrapper in
  a browser, never marking `stdlib:Http` so vbr_stdlib stays out of wasm
  builds);
- the **teaching messages** (`is_browser()`, `surface_name()` — "a Page" vs
  "a browser Screen").

The split/continuation analysis itself (Match/Dim forms, blocking checks) is
written once and shared by all four async shells (Iced `Task::perform`, TUI
thread+channel, Yew `send_future`, Ratzilla `spawn_local`).

## 2. `collect_event_stdlib` made pure

It used to mark `stdlib:<ns>` as it collected (which drives the vbr_stdlib
Cargo dep + features). Marking moved to its callers
(`event_stdlib_imports` marks as before for GUI/TUI), so the web backends can
*inspect* stdlib usage — to fence it with a teaching error — without dragging
vbr_stdlib (ureq, polars…) into a wasm build.

## 3. `HTTP_GET_HELPER` hoisted to `surface.rs`

The gloo-net fetch wrapper was inline in `web.rs`; when the browser Screen
needed the identical helper it became a shared `pub(crate) const` used by both
`web.rs` and `tui.rs`.

## 4. TUI dispatch helpers parameterized by indent

`nav_dispatch` / `enter_dispatch` / `input_dispatch` (arrows, Tab, Enter,
Backspace/typing on the focusable widgets) had the native loop's indentation
hardcoded. They now take a `base: usize` (native passes 6, the web shell 5),
which is the entire reason the browser Screen's focus behaviour is the
*same code* as the terminal's rather than a copy.

## 5. The `web` compile target flag

A `Screen` is one program with two shells, chosen by command — so a target
flag now threads end-to-end:

    vbr::compile_web / compile_module_web
      → transpiler::transpile_module(…, web, …)
        → tui::emit_tui_program(program, web, diags)
          → emit_web_main (Ratzilla shell) | emit_main (crossterm shell)

CLI: `vbr runweb` compiles with `web = true`; `vbr build --web` generates the
browser project without serving; `generate_project`/`compile_path` carry the
flag. `emit_screen` (state struct + `view`) is untouched — only `fn main`
differs between shells.

## 6. Theme helpers shared

`gui.rs`'s `KNOWN_THEMES` / `canonical_theme` went `pub(crate)`; `web.rs` uses
them for Page `Theme` validation and pairs them with `theme_palette` (the same
hex values as Iced 0.13's palettes) → CSS custom properties. One theme
vocabulary across Window and Page.

## 7. The RefCell-guard reborrow (behavioural, worth knowing)

Every browser-Screen closure now opens with

    let mut guard = state.borrow_mut();
    let state = &mut *guard;

because through a `RefMut`'s deref the borrow checker cannot split field
borrows — `state.history.push(state.level)` is E0502 — while a plain `&mut`
allows it. The reborrow restores native `&mut` semantics, so the same event
body compiles under both shells.

## 8. `Compiled` grew web fields

`web_style` (Theme-as-CSS + `Css` blocks) and `web_assets` (local `Image`
files → trunk copy-file directives), produced by `web::page_style` /
`page_assets`, consumed by `generate_project`'s index.html writer. Follows
the existing `web_title` pattern.

## 9. Playground guard

`playground/` (the transpiler compiled to wasm behind a Yew UI) is built by
the compile guard for wasm32 — its structural value is pinning `vbr::compile`
as wasm-clean: a future dependency that doesn't compile for wasm32 fails the
guard instead of silently killing the playground.
