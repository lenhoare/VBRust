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
        Notes {
            notes: iced::widget::text_editor::Content::with_text("Type your notes here…"),
            status: "ready".to_string(),
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
            state.status = format!("{}{}", format!("{}{}", "you typed ", state.notes.text().len()), " characters");
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
