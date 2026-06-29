use iced::widget::{button, column, text};
use iced::Element;

struct Counter {
    count: i64,
}

impl Default for Counter {
    fn default() -> Self {
        Counter {
            count: 0,
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
            state.count += 1;
        }
        Message::Decrement => {
            state.count -= 1;
        }
    }
}

fn view(state: &Counter) -> Element<'_, Message> {
    column![text("Counter"), text(format!("{}", state.count)), button("-").on_press(Message::Decrement), button("+").on_press(Message::Increment)].into()
}

fn main() -> iced::Result {
    eprintln!("[vbr-gui] starting {} — set RUST_LOG=winit=debug,iced=debug for detail", "Counter");
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")))
        .init();
    let result = iced::run("Counter", update, view);
    eprintln!("[vbr-gui] iced::run returned: {:?}", result);
    result
}
