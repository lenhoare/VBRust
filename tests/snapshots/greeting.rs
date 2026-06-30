// A text input bound to state, with a payload-carrying event and a Match in the
// view that reacts to what's typed.

use iced::widget::{column, text, text_input};
use iced::Element;

struct Greeter {
    name: String,
}

impl Default for Greeter {
    fn default() -> Self {
        Greeter {
            name: "".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Rename(String),
}

fn update(state: &mut Greeter, message: Message) {
    match message {
        Message::Rename(value) => {
            state.name = value;
        }
    }
}

fn view(state: &Greeter) -> Element<'_, Message> {
    column![
        text("What's your name?"),
        text_input("type here", &state.name).on_input(Message::Rename),
        {
            let el: Element<'_, Message> = match state.name.as_str() {
                "" => text("Type your name above.").into(),
                _ => text(format!("{}{}", format!("{}{}", "Hello, ", state.name), "!")).into(),
            };
            el
        },
    ].into()
}

fn main() -> iced::Result {
    iced::run("Greeter", update, view)
}
