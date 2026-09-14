// Await Http.Post in the browser — the same fetch wrapper as Http.Get, with a
// JSON body and request headers. CORS still applies: the server must allow the
// origin, or the call comes back as Err.
// Run: vbr runweb examples/web_post.vbr

use yew::prelude::*;
use std::collections::HashMap;

struct Poster {
    endpoint: String,
    status: String,
    reply: String,
    key: String,
}

enum Message {
    SetEndpoint(String),
    Send,
    SendDone(Result<String, String>),
}

impl Component for Poster {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        let endpoint = "https://httpbin.org/post".to_string();
        let status = "idle".to_string();
        let reply = "".to_string();
        let key = "sk-demo-key".to_string();
        Poster {
            endpoint,
            status,
            reply,
            key,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, message: Self::Message) -> bool {
        match message {
            Message::SetEndpoint(value) => {
                {
                    let __vbr_event: Result<(), String> = (|| {
                        self.endpoint = value;
                        Ok(())
                    })();
                    if let Err(__e) = __vbr_event {
                        eprintln!("Error: {}", __e);
                    }
                }
            }
            Message::Send => {
                {
                    let __vbr_event: Result<_, String> = (|| {
                        self.status = "sending…".to_string();
                        let mut headers: HashMap<String, String> = HashMap::new();
                        headers.insert("Authorization".to_string(), format!("Bearer {}", self.key));
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        let body: String = "{\"prompt\": \"hello\"}".to_string();
                        Ok((body, headers))
                    })();
                    match __vbr_event {
                        Err(__e) => {
                            eprintln!("Error: {}", __e);
                        }
                        Ok((body, headers)) => {
                let endpoint = self.endpoint.clone();
                ctx.link().send_future(async move { Message::SendDone(http_post(&endpoint, &body, headers).await) });
                        }
                    }
                }
            }
            Message::SendDone(result) => {
                {
                    let __vbr_event: Result<(), String> = (|| {
                        match result {
                            Ok ( text ) => {
                                self.status = "ok".to_string();
                                self.reply = text;
                            }
                            Err ( message ) => {
                                self.status = "failed".to_string();
                                self.reply = message;
                            }
                        }
                        Ok(())
                    })();
                    if let Err(__e) = __vbr_event {
                        eprintln!("Error: {}", __e);
                    }
                }
            }
        }
        true // state changed — re-render the view
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div class="vbr-column poster" style="display: flex; flex-direction: column;">
                <input
                    class="vbr-textinput"
                    placeholder={"endpoint"}
                    value={self.endpoint.clone()}
                    oninput={ctx.link().callback(|e: InputEvent| Message::SetEndpoint(e.target_unchecked_into::<web_sys::HtmlInputElement>().value()))}
                />
                <button class="vbr-button" onclick={ctx.link().callback(|_| Message::Send)}>{ "POST" }</button>
                <p class="vbr-text">{ format!("{}", self.status) }</p>
                <p class="vbr-text">{ format!("{}", self.reply) }</p>
            </div>
        }
    }
}

/// The browser's `fetch`, shaped like the stdlib's `Http.Post`: the response
/// body on success; any failure (network, CORS, an HTTP error status) as a
/// `String` error.
async fn http_post(
    url: &str,
    body: &str,
    headers: std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let mut builder = gloo_net::http::Request::post(url);
    for (name, value) in headers {
        builder = builder.header(&name, &value);
    }
    let response = builder
        .body(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    response.text().await.map_err(|e| e.to_string())
}

fn main() {
    yew::Renderer::<Poster>::new().render();
}
