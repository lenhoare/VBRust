// A control panel showing every VBR GUI control: Text, Button, TextInput,
// Checkbox, Toggler, Slider, ProgressBar, Radio — plus Row layout, Match and If
// in the view, an enum-backed radio group, and an async Await (Http) event.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Size {
    Small,
    Medium,
    Large,
}

use iced::widget::{button, checkbox, column, progress_bar, radio, row, slider, text, text_input, toggler};
use iced::Element;
use iced::Task;
use vbr_stdlib::{Http};

struct Panel {
    name: String,
    agree: bool,
    dark: bool,
    volume: i32,
    size: Size,
    status: String,
}

impl Default for Panel {
    fn default() -> Self {
        Panel {
            name: "".to_string(),
            agree: false,
            dark: false,
            volume: 50,
            size: Size::Medium,
            status: "ready".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetName(String),
    SetAgree(bool),
    SetDark(bool),
    SetVolume(i32),
    SetSize(Size),
    Fetch,
    FetchDone(Result<String, String>),
}

fn update(state: &mut Panel, message: Message) -> Task<Message> {
    match message {
        Message::SetName(value) => {
            state.name = value;
            Task::none()
        }
        Message::SetAgree(value) => {
            state.agree = value;
            Task::none()
        }
        Message::SetDark(value) => {
            state.dark = value;
            Task::none()
        }
        Message::SetVolume(value) => {
            state.volume = value;
            Task::none()
        }
        Message::SetSize(value) => {
            state.size = value;
            Task::none()
        }
        Message::Fetch => {
            state.status = "loading…".to_string();
            Task::perform(async move { tokio::task::spawn_blocking(move || Http::get("https://example.com")).await.unwrap() }, Message::FetchDone)
        }
        Message::FetchDone(result) => {
            match result {
                Ok ( body ) => {
                    state.status = format!("{}{}", format!("{}{}", "got ", body.len()), " bytes");
                }
                Err ( e ) => {
                    state.status = format!("{}{}", "error: ", e);
                }
            }
            Task::none()
        }
    }
}

fn view(state: &Panel) -> Element<'_, Message> {
    column![
        text("Control Panel"),
        row![
            text("Name:"),
            text_input("your name", &state.name).on_input(Message::SetName),
        ],
        checkbox("I agree to the terms", state.agree).on_toggle(Message::SetAgree),
        toggler(state.dark).label("Dark mode").on_toggle(Message::SetDark),
        text(format!("{}{}", "Volume: ", state.volume)),
        slider(0..=100, state.volume, Message::SetVolume),
        progress_bar((0 as f32)..=(100 as f32), state.volume as f32),
        text("Size:"),
        radio("Small", Size::Small, Some(state.size), Message::SetSize),
        radio("Medium", Size::Medium, Some(state.size), Message::SetSize),
        radio("Large", Size::Large, Some(state.size), Message::SetSize),
        {
            let el: Element<'_, Message> = match state.size {
                Size :: Small => text("compact layout").into(),
                Size :: Medium => text("standard layout").into(),
                Size :: Large => text("spacious layout").into(),
            };
            el
        },
        {
            let el: Element<'_, Message> = if state.agree {
                text("Thanks — you're all set!").into()
            } else {
                text("Please agree to continue.").into()
            };
            el
        },
        button("Fetch a page").on_press(Message::Fetch),
        text(format!("{}", state.status)),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Control Panel", update, view)
}
