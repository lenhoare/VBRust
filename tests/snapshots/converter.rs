// A GUI calling its own function: the Celsius→Fahrenheit conversion lives in a
// helper that the slider's event calls.

fn tofahrenheit(c: i32) -> Result<i32, String> {
    Ok((((c * 9) as f64) / (5 as f64) + 32.0) as i32)
}

use iced::widget::{column, slider, text};
use iced::Element;

struct Converter {
    celsius: i32,
    fahrenheit: i32,
}

impl Default for Converter {
    fn default() -> Self {
        let celsius = 20;
        let fahrenheit = 68;
        Converter {
            celsius,
            fahrenheit,
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
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.celsius = value;
                    state.fahrenheit = tofahrenheit(value)?;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
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
