// Spacing between controls: `Spacing N` puts a uniform gap between a container's
// children, `Padding N` insets the whole container, and `Space Height/Width N`
// adds a one-off blank gap.

use iced::widget::{button, column, row, text};
use iced::Element;

struct Spaced {
    n: i32,
}

impl Default for Spaced {
    fn default() -> Self {
        let n = 0;
        Spaced {
            n,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Up,
    Down,
}

fn update(state: &mut Spaced, message: Message) {
    match message {
        Message::Up => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.n += 1;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
        }
        Message::Down => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.n -= 1;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
        }
    }
}

fn view(state: &Spaced) -> Element<'_, Message> {
    column![
        text("Counter"),
        text(format!("{}", state.n)),
        row![
            button("-").on_press(Message::Down),
            button("+").on_press(Message::Up),
        ].spacing(8),
        iced::widget::Space::with_height(30),
        text("(there is a gap above me)"),
    ].spacing(12).padding(20).into()
}

fn main() -> iced::Result {
    iced::run("Spaced", update, view)
}
