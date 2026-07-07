//! The VBR playground: the whole transpiler (`vbr::compile`) compiled to
//! WebAssembly, wrapped in a two-pane Yew app. Type VBR on the left, read the
//! generated Rust on the right — teaching diagnostics included, entirely in
//! the browser, no server.
//!
//! Everything transpiles here (GUI, TUI, web, dataframes, Python interop);
//! nothing *runs* here — the playground shows the Rust a program becomes, and
//! `vbr run`/`runproject`/`runweb` remain the way to execute it.

use yew::prelude::*;

/// The example picker: a spread across the language, embedded at build time.
const EXAMPLES: &[(&str, &str)] = &[
    ("Hello", include_str!("../../examples/hello.vbr")),
    ("Strings", include_str!("../../examples/string_funcs.vbr")),
    ("Match", include_str!("../../examples/match.vbr")),
    ("Structs & methods", include_str!("../../examples/methods.vbr")),
    ("Enums (sum types)", include_str!("../../examples/sum_types.vbr")),
    ("Iterators", include_str!("../../examples/iterators.vbr")),
    ("Result & Try", include_str!("../../examples/result.vbr")),
    ("Inline Rust", include_str!("../../examples/inline_rust.vbr")),
    ("GUI window (Iced)", include_str!("../../examples/settings.vbr")),
    ("GUI async fetch", include_str!("../../examples/fetch.vbr")),
    ("Terminal app (ratatui)", include_str!("../../examples/tui_counter.vbr")),
    ("Web page (Yew)", include_str!("../../examples/web_counter.vbr")),
    ("DataFrames (polars)", include_str!("../../examples/dataframe_basics.vbr")),
    ("Python interop", include_str!("../../examples/python_scalar.vbr")),
];

struct Playground {
    source: String,
}

enum Message {
    Edit(String),
    Load(String),
}

impl Component for Playground {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Playground {
            source: EXAMPLES[0].1.to_string(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, message: Self::Message) -> bool {
        match message {
            Message::Edit(text) => self.source = text,
            Message::Load(name) => {
                if let Some((_, src)) = EXAMPLES.iter().find(|(n, _)| *n == name) {
                    self.source = src.to_string();
                }
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // The compiler is a pure function of the source — run it per render.
        let result = vbr::compile(&self.source);
        let deps: Vec<String> = result
            .dependencies
            .iter()
            .map(|(krate, version)| format!("{} = \"{}\"", krate, version))
            .collect();
        html! {
            <>
                <header>
                    <h1>{ "VBR Playground" }</h1>
                    <p>{ "VBA syntax in, idiomatic Rust out — transpiled live in your browser." }</p>
                    <label>
                        { "Example: " }
                        <select onchange={ctx.link().callback(|e: Event| {
                            Message::Load(e.target_unchecked_into::<web_sys::HtmlSelectElement>().value())
                        })}>
                            { for EXAMPLES.iter().map(|(name, _)| html! {
                                <option value={*name}>{ *name }</option>
                            }) }
                        </select>
                    </label>
                </header>
                <main>
                    <div class="pane">
                        <h2>{ "VBR" }</h2>
                        <textarea
                            spellcheck="false"
                            value={self.source.clone()}
                            oninput={ctx.link().callback(|e: InputEvent| {
                                Message::Edit(e.target_unchecked_into::<web_sys::HtmlTextAreaElement>().value())
                            })}
                        />
                    </div>
                    <div class="pane">
                        <h2>{ "Rust" }</h2>
                        {
                            if result.diagnostics.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div class="diagnostics">
                                        { for result.diagnostics.iter().map(|d| {
                                            let class = match d.chars().next() {
                                                Some('✘') => "error",
                                                Some('⚠') => "warn",
                                                _ => "note",
                                            };
                                            html! { <p class={class}>{ d.clone() }</p> }
                                        }) }
                                    </div>
                                }
                            }
                        }
                        {
                            if deps.is_empty() || result.has_errors {
                                html! {}
                            } else {
                                html! {
                                    <div class="deps">
                                        { format!("Cargo.toml: {}", deps.join(", ")) }
                                    </div>
                                }
                            }
                        }
                        {
                            if result.has_errors {
                                html! { <pre>{ "— no Rust was produced —" }</pre> }
                            } else {
                                html! { <pre>{ result.rust.clone() }</pre> }
                            }
                        }
                    </div>
                </main>
            </>
        }
    }
}

fn main() {
    yew::Renderer::<Playground>::new().render();
}
