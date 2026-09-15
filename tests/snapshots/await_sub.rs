// Await in a helper Sub: the Event ends with a call to DoFetch, which contains
// the Await. Vinyl flattens that tail call into the event and splits once —
// same kick-off / FetchDone continuation as putting the Await in the Event.

use iced::widget::{button, column, text, text_input};
use iced::Element;
use iced::Task;
use vbr_stdlib::{Http};

struct Fetcher {
    url: String,
    status: String,
}

impl Default for Fetcher {
    fn default() -> Self {
        let url = "https://example.com".to_string();
        let status = "idle".to_string();
        Fetcher {
            url,
            status,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetUrl(String),
    Fetch,
    FetchDone(Result<String, String>),
}

fn update(state: &mut Fetcher, message: Message) -> Task<Message> {
    match message {
        Message::SetUrl(value) => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.url = value;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
            Task::none()
        }
        Message::Fetch => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.status = "loading…".to_string();
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
            let url = state.url.clone();
            Task::perform(async move { tokio::task::spawn_blocking(move || Http::get(&url)).await.unwrap() }, Message::FetchDone)
        }
        Message::FetchDone(result) => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    match result {
                        Ok ( body ) => {
                            state.status = format!("got {} bytes", body.len());
                        }
                        Err ( e ) => {
                            state.status = format!("error: {}", e);
                        }
                    }
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
            Task::none()
        }
    }
}

fn view(state: &Fetcher) -> Element<'_, Message> {
    column![
        text_input("url", &state.url).on_input(Message::SetUrl),
        button("Fetch").on_press(Message::Fetch),
        text(format!("{}", state.status)),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Fetcher", update, view)
}
