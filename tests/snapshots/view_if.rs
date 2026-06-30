// If in the View — show different widgets by condition. ElseIf/Else are optional;
// an If with no Else simply shows nothing when false.

use iced::widget::{button, column, text};
use iced::Element;

struct Gauge {
    level: i32,
}

impl Default for Gauge {
    fn default() -> Self {
        Gauge {
            level: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Up,
    Down,
}

fn update(state: &mut Gauge, message: Message) {
    match message {
        Message::Up => {
            state.level += 1;
        }
        Message::Down => {
            state.level -= 1;
        }
    }
}

fn view(state: &Gauge) -> Element<'_, Message> {
    column![
        text("Level:"),
        text(format!("{}", state.level)),
        button("+").on_press(Message::Up),
        button("-").on_press(Message::Down),
        {
            let el: Element<'_, Message> = if state.level >= 10 {
                text("High!").into()
            } else if state.level <= 0 {
                text("Empty").into()
            } else {
                text("OK").into()
            };
            el
        },
        {
            let el: Element<'_, Message> = if state.level >= 10 {
                text("(you hit the cap)").into()
            } else {
                iced::widget::Space::new(iced::Length::Shrink, iced::Length::Shrink).into()
            };
            el
        },
    ].into()
}

fn main() -> iced::Result {
    iced::run("Gauge", update, view)
}
