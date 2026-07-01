// An Image control shows a picture from a path. Iced's `image` feature is added
// to the project automatically when you use it.

use iced::widget::{column, text};
use iced::Element;

struct Gallery {
    caption: String,
}

impl Default for Gallery {
    fn default() -> Self {
        Gallery {
            caption: "Ferris the crab".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
}

fn update(_state: &mut Gallery, message: Message) {
    match message {
    }
}

fn view(state: &Gallery) -> Element<'_, Message> {
    column![
        text("My picture:"),
        iced::widget::image("ferris.png"),
        text(format!("{}", state.caption)),
    ].spacing(10).padding(20).into()
}

fn main() -> iced::Result {
    iced::run("Gallery", update, view)
}
