// A multi-line text editor (TextArea). The edit handler is generated for you;
// read the typed text with `.Text()`. Here a button counts the characters.

use iced::widget::{button, column, text, text_editor};
use iced::Element;

struct Notes {
    notes: iced::widget::text_editor::Content,
    status: String,
}

impl Default for Notes {
    fn default() -> Self {
        let notes = iced::widget::text_editor::Content::with_text("Type your notes here…");
        let status = "ready".to_string();
        Notes {
            notes,
            status,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Count,
    NotesEdited(iced::widget::text_editor::Action),
}

fn update(state: &mut Notes, message: Message) {
    match message {
        Message::Count => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.status = format!("you typed {} characters", state.notes.text().len());
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
        }
        Message::NotesEdited(action) => {
            state.notes.perform(action);
        }
    }
}

fn view(state: &Notes) -> Element<'_, Message> {
    column![
        text_editor(&state.notes).on_action(Message::NotesEdited),
        button("Count characters").on_press(Message::Count),
        text(format!("{}", state.status)),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Notes", update, view)
}
