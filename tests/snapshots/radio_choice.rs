// Radio buttons backed by a simple enum: the bound field holds the selected
// variant, each Radio offers one option, and On Select reports the choice. The
// enum being Copy is exactly what Iced's radio needs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Size {
    Small,
    Medium,
    Large,
}

use iced::widget::{column, radio, text};
use iced::Element;

struct Chooser {
    choice: Size,
}

impl Default for Chooser {
    fn default() -> Self {
        Chooser {
            choice: Size::Small,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Pick(Size),
}

fn update(state: &mut Chooser, message: Message) {
    match message {
        Message::Pick(value) => {
            state.choice = value;
        }
    }
}

fn view(state: &Chooser) -> Element<'_, Message> {
    column![radio("Small", Size::Small, Some(state.choice), Message::Pick), radio("Medium", Size::Medium, Some(state.choice), Message::Pick), radio("Large", Size::Large, Some(state.choice), Message::Pick), { let el: Element<'_, Message> = match state.choice { Size :: Small => text("You picked small").into(), Size :: Medium => text("You picked medium").into(), Size :: Large => text("You picked large").into(), }; el }].into()
}

fn main() -> iced::Result {
    iced::run("Chooser", update, view)
}
