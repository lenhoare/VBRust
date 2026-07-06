// Async in the browser: Await Http.Get runs on the browser's own fetch, so the
// page never freezes. The event splits — kick-off shows "loading…" right away,
// then the result arrives in a generated continuation that updates the status.
// A direct port of the GUI's fetch.vbr — same blocks, third renderer.
// (The browser enforces CORS: the server must allow cross-origin reads, or the
// request comes back as an Err. api.github.com allows them.)
// Run it with: vbr runweb examples/web_fetch.vbr

use yew::prelude::*;

struct Fetcher {
    url: String,
    status: String,
}

enum Message {
    SetUrl(String),
    Fetch,
    FetchDone(Result<String, String>),
}

impl Component for Fetcher {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Fetcher {
            url: "https://api.github.com/zen".to_string(),
            status: "idle".to_string(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, message: Self::Message) -> bool {
        match message {
            Message::SetUrl(value) => {
                self.url = value;
            }
            Message::Fetch => {
                self.status = "loading…".to_string();
                let url = self.url.clone();
                ctx.link().send_future(async move { Message::FetchDone(http_get(&url).await) });
            }
            Message::FetchDone(result) => {
                match result {
                    Ok ( body ) => {
                        self.status = format!("got {} bytes", body.len());
                    }
                    Err ( e ) => {
                        self.status = format!("error: {}", e);
                    }
                }
            }
        }
        true // state changed — re-render the view
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div class="vbr-column fetcher" style="display: flex; flex-direction: column;">
                <input
                    class="vbr-textinput"
                    placeholder={"url"}
                    value={self.url.clone()}
                    oninput={ctx.link().callback(|e: InputEvent| Message::SetUrl(e.target_unchecked_into::<web_sys::HtmlInputElement>().value()))}
                />
                <button class="vbr-button" onclick={ctx.link().callback(|_| Message::Fetch)}>{ "Fetch" }</button>
                <p class="vbr-text">{ format!("{}", self.status) }</p>
            </div>
        }
    }
}

/// The browser's `fetch`, shaped like the stdlib's `Http.Get`: the response
/// body on success; any failure (network, CORS, an HTTP error status) as a
/// `String` error.
async fn http_get(url: &str) -> Result<String, String> {
    let response = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    response.text().await.map_err(|e| e.to_string())
}

fn main() {
    yew::Renderer::<Fetcher>::new().render();
}
