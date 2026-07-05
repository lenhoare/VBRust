// The first web page: a counter in the browser. Same State/View/Events blocks
// as a Window — a Page compiles to a Yew (WebAssembly) app instead of Iced.
// Run it with: vbr runweb examples/web_counter.vbr

use yew::prelude::*;

struct Counter {
    count: i32,
}

enum Message {
    Increment,
    Decrement,
}

impl Component for Counter {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Counter {
            count: 0,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, message: Self::Message) -> bool {
        match message {
            Message::Increment => {
                self.count += 1;
            }
            Message::Decrement => {
                self.count -= 1;
            }
        }
        true // state changed — re-render the view
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div style="display: flex; flex-direction: column;">
                <p>{ format!("Count: {}", self.count) }</p>
                <div style="display: flex; flex-direction: row;">
                    <button onclick={ctx.link().callback(|_| Message::Increment)}>{ "+" }</button>
                    <button onclick={ctx.link().callback(|_| Message::Decrement)}>{ "-" }</button>
                </div>
            </div>
        }
    }
}

fn main() {
    yew::Renderer::<Counter>::new().render();
}
