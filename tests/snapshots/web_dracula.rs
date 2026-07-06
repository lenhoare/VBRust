// Built-in themes work in the browser: Theme Dracula colors the page with the
// same palette as the desktop Window of the same name. A Css block is the
// escape hatch — real CSS targeting the generated classes (.vbr-button,
// .vbr-text, …) or one page's controls via its name (.counter). A direct port
// of the GUI's dracula.vbr. (web slice 5)
// Run it with: vbr runweb examples/web_dracula.vbr

use yew::prelude::*;

struct Counter {
    count: i32,
}

enum Message {
    Up,
    Down,
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
            Message::Up => {
                self.count += 1;
            }
            Message::Down => {
                self.count -= 1;
            }
        }
        true // state changed — re-render the view
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div class="vbr-column counter" style="display: flex; flex-direction: column; gap: 12px; padding: 20px;">
                <p class="vbr-text">{ "Count" }</p>
                <p class="vbr-text">{ format!("{}", self.count) }</p>
                <div class="vbr-row" style="display: flex; flex-direction: row; gap: 8px;">
                    <button class="vbr-button" onclick={ctx.link().callback(|_| Message::Down)}>{ "-" }</button>
                    <button class="vbr-button" onclick={ctx.link().callback(|_| Message::Up)}>{ "+" }</button>
                </div>
            </div>
        }
    }
}

fn main() {
    yew::Renderer::<Counter>::new().render();
}
