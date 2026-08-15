// Built-in themes: `Theme Dracula` restyles the whole window — no per-control
// work, because Iced themes cascade to every widget.

use iced::widget::{button, column, row, text};
use iced::Element;

struct Counter {
    count: i32,
}

impl Default for Counter {
    fn default() -> Self {
        let count = 0;
        Counter {
            count,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Up,
    Down,
}

fn update(state: &mut Counter, message: Message) {
    match message {
        Message::Up => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.count += 1;
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
                    state.count -= 1;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
        }
    }
}

fn view(state: &Counter) -> Element<'_, Message> {
    column![
        text("Count"),
        text(format!("{}", state.count)),
        row![
            button("-").on_press(Message::Down),
            button("+").on_press(Message::Up),
        ].spacing(8),
    ].spacing(12).padding(20).into()
}

fn main() -> iced::Result {
    iced::application("Dracula Counter", update, view)
        .theme(|_| iced::Theme::Dracula)
        .run()
}
