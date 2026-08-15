use iced::widget::{button, column, text};
use iced::Element;

struct Counter {
    count: i64,
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
    Increment,
    Decrement,
}

fn update(state: &mut Counter, message: Message) {
    match message {
        Message::Increment => {
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
        Message::Decrement => {
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
        text("Counter"),
        text(format!("{}", state.count)),
        button("-").on_press(Message::Decrement),
        button("+").on_press(Message::Increment),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Counter", update, view)
}
