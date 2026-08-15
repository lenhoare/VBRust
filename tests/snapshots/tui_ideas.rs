// tui_ideas.vbr — a Database held in State. A State initialiser may be a
// fallible call (Database.Open, or one of your own Result-returning functions):
// construction then runs *before* the terminal starts — if it fails, you get
// "could not start: <why>" and a clean exit, never a half-alive UI. Events
// just use the open handle (db here is state.db), and passing it to a helper
// function borrows it (&Database).
// A HashMap built inside a plain helper (not an event) still needs the file-top
// `use std::collections::HashMap;`. The surface import scan reads helper bodies
// too now, not only events — before, a Screen whose only HashMap lived in a
// helper compiled to code referencing an unimported type.

fn addidea(db: &Database) -> Result<i64, String> {
    db.execute("CREATE TABLE IF NOT EXISTS ideas (id INTEGER PRIMARY KEY, text TEXT)", vec![])?;
    db.execute("INSERT INTO ideas (text) VALUES (?)", vec!["a fresh idea".to_string()])?;
    let rows: Vec<Json> = db.query("SELECT COUNT(*) AS n FROM ideas", vec![])?;
    Ok(rows[0].get_int("n"))
}

fn bonuspoints() -> Result<i64, String> {
    let mut weights: HashMap<String, i64> = HashMap::new();
    weights.insert("base".to_string(), 3);
    weights.insert("streak".to_string(), 2);
    Ok(weights.len() as i64)
}

use vbr_stdlib::{Json, Database};

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use std::collections::HashMap;

struct Ideas {
    db: Database,
    status: String,
    count: i64,
}

impl Ideas {
    fn init() -> Result<Ideas, String> {
        let db = Database::open("ideas.db")?;
        let status = "a = add an idea, q = quit".to_string();
        let count = 0;
        Ok(Ideas {
            db,
            status,
            count,
        })
    }
}

fn view(state: &Ideas, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Idea Store");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("Ideas stored: {}", state.count)), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[1]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" a ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Add  "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = match Ideas::init() {
        Ok(state) => state,
        Err(message) => {
            eprintln!("could not start: {}", message);
            std::process::exit(1);
        }
    };
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('a') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                #[allow(unused_mut)]
                                let mut n: i64;
                                n = match addidea(&state.db) {
                                    Ok(__vbr_ok) => __vbr_ok,
                                    Err(e) => {
                                        state.status = format!("error: {}", e);
                                        return Ok(());
                                    }
                                };
                                state.count = n;
                                state.status = format!("added — {} ideas now (+{} bonus)", n, bonuspoints()?);
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
                    }
                    KeyCode::Char('q') => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
