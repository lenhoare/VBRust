// database.vbr — SQLite via the stdlib. A Database is a live connection you
// hold and call methods on (like Json/DateTime, not a stateless namespace).
// 
// Params bind to ? placeholders (always as text — column affinity stores them
// typed, so declare columns INTEGER/REAL). NULL goes in the SQL itself:
// VALUES (?, NULL) — a list of strings has no null slot. Query rows come back
// as Json objects keyed by column name, each column with its natural type.
// A ByVal Database param borrows the connection (&Database) — open once,
// hand it around. Inside a Result function, `?` chains the fallible calls.
// text and score are `&str` params. Dropping them straight into the params list
// fills a `Vec<String>`, so each is owned with `.to_string()` for you — no manual
// `.clone()` or `CStr(...)`. A literal element (none here) is owned by the list
// emitter as before.

use vbr_stdlib::{Json, Database};

fn run(db: &Database) -> Result<(), String> {
    db.execute("CREATE TABLE IF NOT EXISTS ideas (id INTEGER PRIMARY KEY, gen INTEGER, text TEXT, score REAL, parent INTEGER)", vec![])?;
    db.execute("DELETE FROM ideas", vec![])?;
    // A root idea has no parent — the NULL is written in the SQL.
    db.execute("INSERT INTO ideas (gen, text, score, parent) VALUES (1, ?, ?, NULL)", vec!["solar tracker".to_string(), "0.82".to_string()])?;
    let root: i64 = db.last_insert_id();
    // A child links to its parent via the fresh rowid — lineage.
    db.execute("INSERT INTO ideas (gen, text, score, parent) VALUES (2, ?, ?, ?)", vec!["improved tracker".to_string(), "0.91".to_string(), root.to_string()])?;
    // Insert through a helper whose text/score arrive as ByVal String params —
    // a `&str` element in the params list, owned into the Vec<String> for you.
    addscored(&db, "wind turbine", "0.75")?;
    let rows: Vec<Json> = db.query("SELECT text, score, parent FROM ideas ORDER BY score DESC", vec![])?;
    for row in &rows {
        let line: String = format!("{} scores {}", (*row).get_string("text")?, (*row).get_float("score")?);
        if (*row).get("parent")?.is_null() {
            println!("{} (a root idea)", line);
        } else {
            println!("{} (child of #{})", line, (*row).get_int("parent")?);
        }
    }
    Ok(())
}

fn addscored(db: &Database, text: &str, score: &str) -> Result<(), String> {
    db.execute("INSERT INTO ideas (gen, text, score, parent) VALUES (3, ?, ?, NULL)", vec![text.to_string(), score.to_string()])?;
    Ok(())
}

fn main() {
    match Database::open("ideas.db") {
        Ok ( db ) => {
            match run(&db) {
                Ok ( _ ) => {
                    println!("done");
                }
                Err ( message ) => {
                    println!("db error: {}", message);
                }
            }
        }
        Err ( message ) => {
            println!("could not open: {}", message);
        }
    }
}
