// Checkbox (bool) and Slider (number) — payload events carry the new value, and
// a Match in the view reacts to the toggle.

use iced::widget::{checkbox, column, slider, text};
use iced::Element;

struct Settings {
    agreed: bool,
    volume: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            agreed: false,
            volume: 50,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetAgreed(bool),
    SetVolume(i32),
}

fn update(state: &mut Settings, message: Message) {
    match message {
        Message::SetAgreed(value) => {
            state.agreed = value;
        }
        Message::SetVolume(value) => {
            state.volume = value;
        }
    }
}

fn view(state: &Settings) -> Element<'_, Message> {
    column![
        checkbox("I agree to the terms", state.agreed).on_toggle(Message::SetAgreed),
        text(format!("Volume: {}", state.volume)),
        slider(0..=100, state.volume, Message::SetVolume),
        {
            let el: Element<'_, Message> = match state.agreed {
                true => text("Thanks — you're all set!").into(),
                false => text("Please agree to continue.").into(),
            };
            el
        },
    ].into()
}

fn main() -> iced::Result {
    iced::run("Settings", update, view)
}
