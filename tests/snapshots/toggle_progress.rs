// Toggler (an on/off switch) and ProgressBar (a read-only gauge). The slider
// drives the progress bar, and the toggler flips a boolean shown via If.

use iced::widget::{column, progress_bar, slider, text, toggler};
use iced::Element;

struct Panel {
    enabled: bool,
    level: i32,
}

impl Default for Panel {
    fn default() -> Self {
        Panel {
            enabled: false,
            level: 30,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetEnabled(bool),
    SetLevel(i32),
}

fn update(state: &mut Panel, message: Message) {
    match message {
        Message::SetEnabled(value) => {
            state.enabled = value;
        }
        Message::SetLevel(value) => {
            state.level = value;
        }
    }
}

fn view(state: &Panel) -> Element<'_, Message> {
    column![
        toggler(state.enabled).label("Enabled").on_toggle(Message::SetEnabled),
        {
            let el: Element<'_, Message> = if state.enabled {
                text("Running").into()
            } else {
                text("Paused").into()
            };
            el
        },
        slider(0..=100, state.level, Message::SetLevel),
        progress_bar((0 as f32)..=(100 as f32), state.level as f32),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Panel", update, view)
}
