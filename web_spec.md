# VBR Web Specification

A `Page` is a **browser application**: the same State/View/Events model as a
`Window` (GUI) and a `Screen` (TUI), rendered to real HTML in a real browser.
It is backed by the Rust **Yew** crate (version-pinned, like Iced 0.13),
compiled to **WebAssembly**, and served by **trunk**.

> Status: **slice 1 BUILT** (2026-07-05) — `Page`/`Title`/`State`/`View`
> (`Text`, `Button`, `Column`, `Row` with `Spacing`/`Padding`)/`Event`,
> `vbr runweb`. Later slices are listed in §8 and not yet built.

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
- `Theme` is **not** a Page concept (a browser styles with CSS — later slice);
  using it is an error.
- A program is one kind of app: mixing `Page` with `Window`/`Screen` is an
  error.

## 3. V1 controls (slice 1)

| Control | Lowers to |
|---------|-----------|
| `Text <expr>` | `<p>{ … }</p>` (literals as-is, `&` concatenation via `format!`) |
| `Button "label" … On Click <Event>` | `<button onclick={ctx.link().callback(…)}>` |
| `Column` / `Row` (+ `Spacing n`, `Padding n`) | flexbox `<div>` (`gap`/`padding` in px) |

Anything else — including `Length`/`Fill` sizing — is a teaching error naming
the slice it arrives in.

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

## 5. What a Page cannot do (yet)

Each is a teaching error today:

- **`Await` / async events** — Page events are synchronous in slice 1. Browser
  async (`spawn_local`) arrives with the web `Http` story.
- **The stdlib** — a browser sandbox has no filesystem, and blocking calls
  would freeze the page. A web-friendly `Http` (the browser's `fetch` via
  `gloo-net`) arrives in a later slice; `FileSystem`/`DataFrame` don't apply
  in a browser.
- **Event parameters** — they arrive with the input controls (`TextInput`,
  `Checkbox`) in slice 2.

## 6. Testing

`examples/web_counter.vbr` is snapshot-tested (TRANSPILE_ONLY) and built for
real by the compile guard (`cargo test -- --ignored`) whenever the wasm target
is installed — skipped with a notice otherwise.

## 7. Backend mapping

| VBR | Yew |
|-----|-----|
| `Page X` | `struct X` + `impl Component for X` |
| `State` fields | fields on `X` (`self.field`) |
| `Dim` initialisers | `fn create` |
| `Event E` | `Message::E` + an `update` arm returning `true` |
| `View` | `fn view` → `html!` |
| `X.Run` in `Main` | `yew::Renderer::<X>::new().render()` |

## 8. Deferred (later slices)

1. **Input round-trip** — `TextInput` (`oninput` → `Message(String)`),
   `Checkbox`; event parameters.
2. **View logic** — `Match`/`If` in the view, `Slider`, `ProgressBar`, `Image`,
   sizing.
3. **Async** — `Await` via `spawn_local`; web `Http` over the browser's fetch
   (`gloo-net`).
4. **Styling** — a CSS story beyond inline flexbox; maybe `Theme`.
