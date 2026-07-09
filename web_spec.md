# VBR Web Specification

A `Page` is a **browser application**: the same State/View/Events model as a
`Window` (GUI) and a `Screen` (TUI), rendered to real HTML in a real browser.
It is backed by the Rust **Yew** crate (version-pinned, like Iced 0.13),
compiled to **WebAssembly**, and served by **trunk**.

> Status: **slices 1–5 BUILT** (2026-07-06) — `Page`/`Title`/`State`/`View`
> (`Text`, `Button`, `TextInput`, `Checkbox`, `Slider`, `ProgressBar`, `Image`,
> `Match`/`If` in the view, `Column`/`Row` with `Spacing`/`Padding` and
> `Length`/`Fill` sizing)/`Event` including payload events, async events
> (`Await Http.Get` on the browser's fetch), styling (`Theme`, `Css` blocks,
> stable `vbr-*` classes) and local `Image` assets, `vbr runweb`.

---

## 1. Design goals

- **The same machine, a third dress.** A `Page` uses the very blocks you know
  from `Window` and `Screen`. A Yew *struct component* is VBR's model verbatim —
  a state struct, a `Message` enum, an `update` that changes state, a `view`
  derived from it — so the generated Rust reads like the GUI output and the
  knowledge transfers.
- **Real DOM, readable output.** The view lowers to Yew's `html!` macro —
  actual HTML elements (`<p>`, `<button>`, flexbox `<div>`s), not a canvas.
- **Shareable.** The point of a web page: run `vbr runweb`, get a URL.
- **The framework is an implementation detail.** You write VBR, not Yew; the
  backend pins Yew 0.21 and could swap renderers without user code changing.

## 2. The model

```vb
Page Counter
    Title "Counter"                 ' the browser-tab title
    State
        Dim count As Integer = 0
    End State
    View
        Column
            Text "Count: " & count
            Button "+"
                On Click Increment
            End Button
        End Column
    End View
    Event Increment
        count += 1
    End Event
End Page

Function Main()
    Counter.Run
End Function
```

- The component struct is named after the page; state fields live on it
  (`self.count` in the generated Rust).
- Each `Event` becomes a `Message` variant and an `update` arm; `update`
  returns `true`, telling Yew to re-render the view from the new state.
- Event bodies are ordinary VBR — the same resolver pass as a function body
  (string/numeric coercions, iterator chains, teaching diagnostics), shared
  with the GUI/TUI backends (`src/surface.rs`).
- `Theme` works like the GUI's (`Theme Dracula`) — the palette becomes CSS in
  the generated page (§6).
- A program is one kind of app: mixing `Page` with `Window`/`Screen` is an
  error.

## 3. V1 controls

| Control | Lowers to |
|---------|-----------|
| `Text <expr>` | `<p>{ … }</p>` (literals as-is, `&` concatenation via `format!`) |
| `Button "label" … On Click <Event>` | `<button onclick={ctx.link().callback(…)}>` |
| `TextInput "placeholder", field … On Input <Event>` | a controlled `<input>` — `value` from state, `oninput` sends the new text |
| `Checkbox "label", field … On Toggle <Event>` | `<input type="checkbox">` inside its `<label>`, `onchange` sends the new state |
| `Slider min..=max, field … On Change <Event>` | `<input type="range">` — the dragged value is cast to the field's type |
| `ProgressBar min..=max, field` | `<progress>` (a non-zero `min` shifts value and max — HTML progress starts at 0) |
| `Image <path>` | `<img src=…>` — an absolute `https://…` URL, or a local file copied into the site (§6) |
| `Match` / `If` in the view | a Rust `match`/`if` choosing an `html!` fragment (no `Else` → renders nothing) |
| `Column` / `Row` (+ `Spacing n`, `Padding n`) | flexbox `<div>` (`gap`/`padding` in px) |
| `Length n` / `Fill [w]` before a child | a wrapping `<div>` — fixed px on the container's axis / CSS `flex: w` |

The controls are exactly the GUI's: the same syntax, the same payload-carrying
events (`Event Rename(value As String)` / `Event SetVolume(value As Integer)`),
the same binding rules (a `TextInput` binds a `String` field, a `Checkbox` a
`Boolean`, a `Slider`/`ProgressBar` a numeric — anything else is an error).
Reading a typed value needs the input's DOM element, so the project build adds
`web-sys` (feature `HtmlInputElement`) automatically when an input is used.
`Percent`/`Min` sizing stays Screen (TUI) only, as in the GUI.

Anything else is a teaching error naming what a Page supports so far.

## 4. Running — `vbr runweb`

```sh
vbr runweb examples/web_counter.vbr
```

Generates the cargo project in `build/` (with the `index.html` trunk serves;
the `Title` becomes the tab title), builds it for `wasm32-unknown-unknown`
(build errors are translated back to `.vbr` lines), then hands over to
`trunk serve --open`.

One-time setup, each checked up front with a friendly error:

```sh
rustup target add wasm32-unknown-unknown   # the WebAssembly compile target
cargo install trunk --locked               # the wasm bundler + dev server
```

`vbr run`/`vbr runproject` on a `Page` program redirect you to `runweb`;
`vbr build` generates the project without serving it.

## 5. Async events — `Await Http.Get`

The same `Await` you know from the GUI/TUI, on the browser's own machinery:

```vb
Event Fetch
    status = "loading…"
    Match Await Http.Get(url)
        Ok(body) => status = "got " & body.len() & " bytes"
        Err(e) => status = "error: " & e
    End Match
End Event
```

- The event **splits** exactly as in a `Window`: everything before the `Await`
  runs in the kick-off (so `"loading…"` shows immediately), and the code after
  it lands in a generated `<Event>Done(result)` continuation that runs when
  the response arrives. `Dim r As Result<String, String> = Await …` works too.
- The kick-off hands the future to the component with
  `ctx.link().send_future(…)` — Yew's equivalent of Iced's `Task::perform`.
- **`Http.Get` here is the browser's `fetch`**, not the native stdlib (which
  can't compile to wasm): the transpiler generates a small `http_get` wrapper
  over `gloo-net`, shaped like the stdlib's — the body on success, any failure
  (network, an HTTP error status) as a `String` error. The `gloo-net`
  dependency is added automatically.
- **CORS**: a browser only lets a page read a cross-origin response if the
  server allows it (`Access-Control-Allow-Origin`). A server that doesn't
  comes back as an `Err` — `api.github.com` allows it, `example.com` doesn't.
- Calling `Http.Get` *without* `Await` is the same teaching error as in the
  GUI: it would freeze the page.

## 6. Styling — `Theme`, `Css`, and the `vbr-*` classes

Three layers, smallest first:

**Stable classes.** Every generated element carries a class naming its kind:
`vbr-text`, `vbr-button`, `vbr-textinput`, `vbr-checkbox`, `vbr-slider`,
`vbr-progressbar`, `vbr-image`, `vbr-column`, `vbr-row`. The page's root
container additionally carries the page's own (lowercased) name, so a
stylesheet can say `.vbr-button` (every button) or `.counter .vbr-button`
(this page's buttons). These names are a stable contract. Layout stays inline
(your `Spacing`/`Padding`/`Length`/`Fill` numbers); colors and fonts are
never inlined, so CSS has full authority over appearance.

**`Theme` — the simple case.** The same line as the GUI:

```vb
Page Counter
    Title "Dracula Counter"
    Theme Dracula
```

The theme becomes a `<style>` block in the generated `index.html`: the
palette of the **Iced theme of the same name** (so the browser Dracula is the
desktop Dracula) as CSS custom properties — `--vbr-background`, `--vbr-text`,
`--vbr-primary` — plus base rules for the body and the `vbr-*` controls. All
22 GUI theme names work; an unknown name is the same teaching error.

**`Css … End Css` — the escape hatch.** Real CSS, verbatim, at the top level
of the `.vbr` file (like an inline `Rust` block — the inside isn't VBR):

```vb
Css
.counter .vbr-text {
    font-size: 24px;
}
End Css
```

It lands in `index.html` *after* the theme's rules, so it overrides them —
tweak one thing or replace the look entirely. A `Css` block in a program with
no `Page` is a teaching error.

**Local `Image` assets.** An `Image "logo.png"` whose path is a plain local
file name becomes a trunk copy directive in `index.html`, so the file is
copied into the served site. Keep the file next to the `.vbr`. An absolute
`https://…` URL needs no copying, and a *computed* path can't be detected at
compile time — use a literal for local files.

`examples/web_dracula.vbr` — the GUI's dracula.vbr ported, plus a `Css`
block — shows all of it.

## 7. What a Page cannot do (yet)

Each is a teaching error today:

- **The stdlib** — a browser sandbox has no filesystem, and vbr_stdlib doesn't
  compile to wasm. The one door is `Await Http.Get` in an event (§5);
  `FileSystem`/`DataFrame` don't apply in a browser.
- **`Await` on your own functions** — the browser is single-threaded, with no
  background thread to run a synchronous function on (the GUI uses
  `spawn_blocking`; wasm has no equivalent).
- **Fallible `State` initialisers** (`Dim db As Database = Database.Open(…)`) —
  native Windows/Screens build such state before launch and bail cleanly on
  failure; a browser component has no startup moment to fail in (and the
  stdlib isn't on wasm anyway). Give the field a plain initial value.

## 8. Testing

`examples/web_counter.vbr` (slice 1), `examples/web_greeting.vbr` (slice 2:
inputs, payload events), `examples/web_settings.vbr` (slice 3: Match/If,
Slider, ProgressBar), `examples/web_fetch.vbr` (slice 4: `Await Http.Get`),
and `examples/web_dracula.vbr` (slice 5: Theme + Css) are snapshot-tested
(TRANSPILE_ONLY); greeting/settings/fetch are also built for real by the
compile guard (`cargo test -- --ignored`) whenever the wasm target is
installed — skipped with a notice otherwise. (Dracula's Theme/Css land in
`index.html`, which a cargo build doesn't see.)

## 9. Backend mapping

| VBR | Yew |
|-----|-----|
| `Page X` | `struct X` + `impl Component for X` |
| `State` fields | fields on `X` (`self.field`) |
| `Dim` initialisers | `fn create` |
| `Event E` | `Message::E` + an `update` arm returning `true` |
| async `Event E` | kick-off arm (`ctx.link().send_future`) + `Message::EDone` continuation arm |
| `Await Http.Get(url)` | generated `http_get` wrapper over gloo-net's fetch |
| `View` | `fn view` → `html!` |
| `X.Run` in `Main` | `yew::Renderer::<X>::new().render()` |

## 10. Deferred (later slices)

1. **More of `Http`** — `Await Http.Post` (the fetch wrapper generalises
   easily) once the native side awaits it too.
