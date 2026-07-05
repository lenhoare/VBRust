// View logic in the browser: Match and If choose what to show, a Slider drags a
// number into state, and a ProgressBar mirrors it. A direct port of the GUI's
// settings.vbr — same blocks, third renderer. (web slice 3)
// Run it with: vbr runweb examples/web_settings.vbr

use yew::prelude::*;

struct Settings {
    agreed: bool,
    volume: i32,
}

enum Message {
    SetAgreed(bool),
    SetVolume(i32),
}

impl Component for Settings {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Settings {
            agreed: false,
            volume: 50,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, message: Self::Message) -> bool {
        match message {
            Message::SetAgreed(value) => {
                self.agreed = value;
            }
            Message::SetVolume(value) => {
                self.volume = value;
            }
        }
        true // state changed — re-render the view
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div style="display: flex; flex-direction: column;">
                <label>
                    <input
                        type="checkbox"
                        checked={self.agreed}
                        onchange={ctx.link().callback(|e: Event| Message::SetAgreed(e.target_unchecked_into::<web_sys::HtmlInputElement>().checked()))}
                    />
                    { "I agree to the terms" }
                </label>
                <p>{ format!("Volume: {}", self.volume) }</p>
                <input
                    type="range"
                    min="0"
                    max="100"
                    value={self.volume.to_string()}
                    oninput={ctx.link().callback(|e: InputEvent| Message::SetVolume(e.target_unchecked_into::<web_sys::HtmlInputElement>().value_as_number() as i32))}
                />
                <progress max="100" value={self.volume.to_string()}></progress>
                {
                    if self.volume > 80 {
                        html! {
                            <p>{ "That's loud!" }</p>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    match self.agreed {
                        true => html! {
                            <p>{ "Thanks — you're all set!" }</p>
                        },
                        false => html! {
                            <p>{ "Please agree to continue." }</p>
                        },
                    }
                }
            </div>
        }
    }
}

fn main() {
    yew::Renderer::<Settings>::new().render();
}
