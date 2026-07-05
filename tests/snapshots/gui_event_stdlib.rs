// The stdlib inside a Window event: event bodies run the same resolver pass as
// function bodies, so multi-word methods (AddDays -> add_days) and string
// coercions work here too.

use iced::widget::{button, column, text};
use iced::Element;
use vbr_stdlib::{DateTime};

struct Clock {
    label: String,
}

impl Default for Clock {
    fn default() -> Self {
        Clock {
            label: "press stamp".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Stamp,
}

fn update(state: &mut Clock, message: Message) {
    match message {
        Message::Stamp => {
            let now: DateTime = DateTime::now();
            let later: DateTime = now.add_days(30);
            state.label = format!("in 30 days: {}", later.format("%Y-%m-%d"));
        }
    }
}

fn view(state: &Clock) -> Element<'_, Message> {
    column![
        text(format!("{}", state.label)),
        button("Stamp").on_press(Message::Stamp),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Clock", update, view)
}
