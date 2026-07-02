// Await your own function: a (pretend-slow) computation runs off the UI thread
// via Await, and its Result comes back in the generated continuation. No stdlib
// needed — VBR knows the function's return type, so it can build the message.

fn sumto(n: i32) -> Result<i32, String> {
    if n < 0 {
        return Err("negative input".to_string());
    }
    let mut total: i32 = 0;
    let mut i: i32 = 1;
    while i <= n {
        total = total + i;
        i = i + 1;
    }
    Ok(total)
}

use iced::widget::{button, column, slider, text};
use iced::Element;
use iced::Task;

struct Worker {
    input: i32,
    status: String,
}

impl Default for Worker {
    fn default() -> Self {
        Worker {
            input: 5,
            status: "idle".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetInput(i32),
    Compute,
    ComputeDone(Result<i32, String>),
}

fn update(state: &mut Worker, message: Message) -> Task<Message> {
    match message {
        Message::SetInput(value) => {
            state.input = value;
            Task::none()
        }
        Message::Compute => {
            state.status = "working…".to_string();
            let input = state.input.clone();
            Task::perform(async move { tokio::task::spawn_blocking(move || sumto(input)).await.unwrap() }, Message::ComputeDone)
        }
        Message::ComputeDone(result) => {
            match result {
                Ok ( result ) => {
                    state.status = format!("sum = {}", result);
                }
                Err ( e ) => {
                    state.status = format!("error: {}", e);
                }
            }
            Task::none()
        }
    }
}

fn view(state: &Worker) -> Element<'_, Message> {
    column![
        text("Input:"),
        text(format!("{}", state.input)),
        slider(0..=50, state.input, Message::SetInput),
        button("Sum 1..n").on_press(Message::Compute),
        text(format!("{}", state.status)),
    ].spacing(10).padding(20).into()
}

fn main() -> iced::Result {
    iced::run("Worker", update, view)
}
