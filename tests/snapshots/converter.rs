// A GUI calling its own function: the Celsius→Fahrenheit conversion lives in a
// helper that the slider's event calls.

fn to_fahrenheit(c: i32) -> i32 {
    c * 9 / 5 + 32
}

use iced::widget::{column, slider, text};
use iced::Element;

struct Converter {
    celsius: i32,
    fahrenheit: i32,
}

impl Default for Converter {
    fn default() -> Self {
        Converter {
            celsius: 20,
            fahrenheit: 68,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetCelsius(i32),
}

fn update(state: &mut Converter, message: Message) {
    match message {
        Message::SetCelsius(value) => {
            state.celsius = value;
            state.fahrenheit = to_fahrenheit(value);
        }
    }
}

fn view(state: &Converter) -> Element<'_, Message> {
    column![
        text("Celsius:"),
        text(format!("{}", state.celsius)),
        slider(0..=100, state.celsius, Message::SetCelsius),
        text("Fahrenheit:"),
        text(format!("{}", state.fahrenheit)),
    ].spacing(10).padding(20).into()
}

fn main() -> iced::Result {
    iced::run("Converter", update, view)
}
