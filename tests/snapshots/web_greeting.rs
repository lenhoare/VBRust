// The input round-trip in the browser: a TextInput fires its event on every
// keystroke with the new text, a Checkbox with its new state — payload events
// (`value As String`) become Message variants carrying data. (web slice 2)
// Run it with: vbr runweb examples/web_greeting.vbr

use yew::prelude::*;

struct Greeter {
    name: String,
    shout: bool,
}

enum Message {
    Rename(String),
    SetShout(bool),
    Clear,
}

impl Component for Greeter {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Greeter {
            name: "".to_string(),
            shout: false,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, message: Self::Message) -> bool {
        match message {
            Message::Rename(value) => {
                if self.shout {
                    self.name = value.to_uppercase();
                } else {
                    self.name = value;
                }
            }
            Message::SetShout(value) => {
                self.shout = value;
            }
            Message::Clear => {
                self.name = "".to_string();
            }
        }
        true // state changed — re-render the view
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div style="display: flex; flex-direction: column;">
                <p>{ "What's your name?" }</p>
                <input
                    placeholder={"type here"}
                    value={self.name.clone()}
                    oninput={ctx.link().callback(|e: InputEvent| Message::Rename(e.target_unchecked_into::<web_sys::HtmlInputElement>().value()))}
                />
                <label>
                    <input
                        type="checkbox"
                        checked={self.shout}
                        onchange={ctx.link().callback(|e: Event| Message::SetShout(e.target_unchecked_into::<web_sys::HtmlInputElement>().checked()))}
                    />
                    { "Shout it" }
                </label>
                <p>{ format!("Hello, {}!", self.name) }</p>
                <button onclick={ctx.link().callback(|_| Message::Clear)}>{ "Clear" }</button>
            </div>
        }
    }
}

fn main() {
    yew::Renderer::<Greeter>::new().render();
}
