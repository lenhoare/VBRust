use iced::widget::{button, column, text};
use iced::Element;

struct Layout {
    clicks: i32,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            clicks: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Bump,
}

fn update(state: &mut Layout, message: Message) {
    match message {
        Message::Bump => {
            state.clicks += 1;
        }
    }
}

fn view(state: &Layout) -> Element<'_, Message> {
    column![
        iced::widget::container(text("Header — fixed 40px tall")).height(iced::Length::Fixed(40.0)),
        iced::widget::container(text(format!("{}{}", "Body fills the remaining space. Clicks: ", state.clicks))).height(iced::Length::Fill),
        iced::widget::container(button("Footer button").on_press(Message::Bump)).height(iced::Length::Fixed(30.0)),
    ].spacing(10).padding(10).into()
}

fn main() -> iced::Result {
    iced::run("Layout Sizing", update, view)
}
