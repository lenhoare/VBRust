// tui_ideas.vbr — a Database held in State. A State initialiser may be a
// fallible call (Database.Open, or one of your own Result-returning functions):
// construction then runs *before* the terminal starts — if it fails, you get
// "could not start: <why>" and a clean exit, never a half-alive UI. Events
// just use the open handle (db here is state.db), and passing it to a helper
// function borrows it (&Database).

fn addidea(db: &Database) -> Result<i64, String> {
    db.execute("CREATE TABLE IF NOT EXISTS ideas (id INTEGER PRIMARY KEY, text TEXT)", vec![])?;
    db.execute("INSERT INTO ideas (text) VALUES (?)", vec!["a fresh idea".to_string()])?;
    let rows: Vec<Json> = db.query("SELECT COUNT(*) AS n FROM ideas", vec![])?;
    Ok(rows[0].get_int("n")?)
}

use vbr_stdlib::{Json, Database};

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Ideas {
    db: Database,
    status: String,
    count: i64,
}

impl Ideas {
    fn init() -> Result<Ideas, String> {
        Ok(Ideas {
            db: Database::open("ideas.db")?,
            status: "a = add an idea, q = quit".to_string(),
            count: 0,
        })
    }
}

fn view(state: &Ideas, frame: &mut Frame) {
    let block = Block::bordered().title("Idea Store");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("Ideas stored: {}", state.count)), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[1]);
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
                        match addidea(&state.db) {
                            Ok ( n ) => {
                                state.count = n;
                                state.status = format!("added — {} ideas now", n);
                            }
                            Err ( e ) => {
                                state.status = format!("error: {}", e);
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
