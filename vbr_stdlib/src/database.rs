//! Wraps `rusqlite` — SQLite, the embedded SQL database. A `Database` is a live
//! connection you hold and call methods on (like `Json`/`DateTime`, not a
//! stateless namespace). The `bundled` feature compiles SQLite from source, so
//! nothing needs installing.
//!
//! Parameters bind to `?` placeholders positionally, always as text — SQLite's
//! *column affinity* converts them into typed columns (declare columns
//! `INTEGER`/`REAL` and `"42"` is stored as the number 42). To store a NULL,
//! write it in the SQL itself (`VALUES (?, NULL)`) — a parameter list of
//! strings has no null slot.
//!
//! Query rows come back as `Json` objects (one per row, keyed by column name),
//! carrying each column's natural storage type: INTEGER → a JSON integer,
//! REAL → a float, TEXT → a string, NULL → JSON null. So `GetInt`/`GetFloat`/
//! `GetString` read typed values directly, and `IsNull` spots a NULL.

use crate::json::Json;

pub struct Database(rusqlite::Connection);

impl Database {
    /// Open (or create) the database file at `path`.
    /// VBA: like opening an ADO Connection, minus the connection string.
    pub fn open(path: &str) -> Result<Database, String> {
        rusqlite::Connection::open(path).map(Database).map_err(|e| e.to_string())
    }

    /// Run a statement that returns no rows — CREATE / INSERT / UPDATE /
    /// DELETE. Returns how many rows it changed (0 for DDL like CREATE).
    /// VBA: Connection.Execute
    pub fn execute(&self, sql: &str, params: Vec<String>) -> Result<i64, String> {
        self.0
            .execute(sql, rusqlite::params_from_iter(params.iter()))
            .map(|n| n as i64)
            .map_err(|e| e.to_string())
    }

    /// Run a SELECT and collect every row as a `Json` object keyed by column
    /// name. VBA: Connection.Execute returning a Recordset — but a plain list
    /// of values, not a cursor.
    pub fn query(&self, sql: &str, params: Vec<String>) -> Result<Vec<Json>, String> {
        let mut stmt = self.0.prepare(sql).map_err(|e| e.to_string())?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt
            .query(rusqlite::params_from_iter(params.iter()))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let mut object = serde_json::Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match row.get_ref(i).map_err(|e| e.to_string())? {
                    rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => serde_json::Value::from(n),
                    rusqlite::types::ValueRef::Real(f) => serde_json::Value::from(f),
                    rusqlite::types::ValueRef::Text(t) => serde_json::Value::from(
                        std::str::from_utf8(t).map_err(|e| e.to_string())?,
                    ),
                    rusqlite::types::ValueRef::Blob(_) => {
                        return Err(format!(
                            "column '{}' holds a BLOB — store text or numbers, or read \
                             the blob with inline Rust",
                            name
                        ));
                    }
                };
                object.insert(name.clone(), value);
            }
            out.push(Json::from_value(serde_json::Value::Object(object)));
        }
        Ok(out)
    }

    /// The rowid of the most recent successful INSERT on this connection —
    /// e.g. to link child rows to a parent just inserted.
    pub fn last_insert_id(&self) -> i64 {
        self.0.last_insert_rowid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A unique temp-file path per test — hermetic, like http's loopback server.
    fn temp_db() -> String {
        static N: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir()
            .join(format!(
                "vbr_db_test_{}_{}.sqlite",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn round_trip_typed_rows() {
        let path = temp_db();
        let db = Database::open(&path).unwrap();
        db.execute(
            "CREATE TABLE ideas (id INTEGER PRIMARY KEY, gen INTEGER, text TEXT, \
             score REAL, parent INTEGER)",
            vec![],
        )
        .unwrap();
        // Numbers go in as text; column affinity stores them typed. NULL is
        // written in the SQL — a Vec<String> has no null slot.
        db.execute(
            "INSERT INTO ideas (gen, text, score, parent) VALUES (?, ?, ?, NULL)",
            vec!["1".to_string(), "solar tracker".to_string(), "0.82".to_string()],
        )
        .unwrap();
        let rows = db
            .query("SELECT * FROM ideas WHERE gen = ?", vec!["1".to_string()])
            .unwrap();
        assert_eq!(rows.len(), 1);
        // Typed reads — the INTEGER/REAL columns come back as numbers, not text.
        assert_eq!(rows[0].get_int("gen").unwrap(), 1);
        assert_eq!(rows[0].get_string("text").unwrap(), "solar tracker");
        assert!((rows[0].get_float("score").unwrap() - 0.82).abs() < 1e-9);
        assert!(rows[0].get("parent").unwrap().is_null());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn execute_counts_and_last_insert_id() {
        let path = temp_db();
        let db = Database::open(&path).unwrap();
        db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, x INTEGER)", vec![]).unwrap();
        db.execute("INSERT INTO t (x) VALUES (?)", vec!["10".to_string()]).unwrap();
        let first = db.last_insert_id();
        db.execute("INSERT INTO t (x) VALUES (?)", vec!["20".to_string()]).unwrap();
        assert_eq!(db.last_insert_id(), first + 1);
        let changed =
            db.execute("UPDATE t SET x = x + 1", vec![]).unwrap();
        assert_eq!(changed, 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn errors_are_strings() {
        let path = temp_db();
        let db = Database::open(&path).unwrap();
        // Bad SQL comes back as an Err, not a panic.
        assert!(db.execute("NOT REAL SQL", vec![]).is_err());
        assert!(db.query("SELECT * FROM missing", vec![]).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
