// State takes the same types as Dim in Main: HashMap, Option, tuples, arrays.

use iced::widget::{button, column, text};
use iced::Element;
use std::collections::HashMap;

struct Roster {
    ages: HashMap<String, i64>,
    picked: Option<i64>,
    pair: (i64, i64),
    scores: [i32; 3],
    label: String,
}

impl Default for Roster {
    fn default() -> Self {
        let ages = HashMap::new();
        let picked = None;
        let pair = (0, 0);
        let scores = [0; 3];
        let label = "empty".to_string();
        Roster {
            ages,
            picked,
            pair,
            scores,
            label,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Add,
}

fn update(state: &mut Roster, message: Message) {
    match message {
        Message::Add => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.ages.insert("Ada".to_string(), 36);
                    state.picked = Some(36);
                    state.pair = (1, 2);
                    state.scores[0] = 10;
                    state.label = format!("n = {}", state.ages.len());
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
        }
    }
}

fn view(state: &Roster) -> Element<'_, Message> {
    column![
        text(format!("{}", state.label)),
        button("Add").on_press(Message::Add),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Roster", update, view)
}
