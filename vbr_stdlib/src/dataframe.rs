//! Native dataframes for VBR, wrapping the `polars` crate. A `DataFrame` is a
//! table of typed columns you read, transform, and write.
//!
//! Transforms are expressed with polars **expressions** — `col`, `lit`,
//! `when/then/otherwise` — which VBR generates from *column formulas* (e.g.
//! `price * qty`, `IIf(age >= 18, "adult", "minor")`). Each transform returns a
//! new eager `DataFrame`; internally it runs through polars' lazy engine and
//! collects, so the model stays simple (each step materialises) while still using
//! the expression API. Reading this file is a good way to learn real polars.

use polars::prelude::*;

// The expression builders VBR's generated code calls directly.
pub use polars::prelude::{col, lit, when};

/// A table of columns. A thin newtype over polars' `DataFrame` that hides
/// `PolarsResult` and builder boilerplate behind clean, VBR-friendly methods.
#[derive(Clone)]
pub struct DataFrame(polars::prelude::DataFrame);

impl DataFrame {
    /// Read a CSV file (with a header row) into a frame.
    pub fn read_csv(path: &str) -> DataFrame {
        let df = CsvReadOptions::default()
            .with_has_header(true)
            .try_into_reader_with_file_path(Some(path.into()))
            .expect("could not open the CSV file")
            .finish()
            .expect("could not read the CSV file");
        DataFrame(df)
    }

    /// Keep the rows where the boolean column expression is true.
    pub fn filter(&self, mask: Expr) -> DataFrame {
        DataFrame(
            self.0
                .clone()
                .lazy()
                .filter(mask)
                .collect()
                .expect("filter failed"),
        )
    }

    /// Add (or replace) a column computed by an expression.
    pub fn with_column(&self, name: &str, e: Expr) -> DataFrame {
        DataFrame(
            self.0
                .clone()
                .lazy()
                .with_column(e.alias(name))
                .collect()
                .expect("with_column failed"),
        )
    }

    /// Keep only the named columns, in the given order.
    pub fn select(&self, cols: &[&str]) -> DataFrame {
        let exprs: Vec<Expr> = cols.iter().map(|c| col(*c)).collect();
        DataFrame(
            self.0
                .clone()
                .lazy()
                .select(exprs)
                .collect()
                .expect("select failed"),
        )
    }

    /// Sort ascending by one column.
    pub fn sort(&self, name: &str) -> DataFrame {
        DataFrame(
            self.0
                .clone()
                .lazy()
                .sort([name], Default::default())
                .collect()
                .expect("sort failed"),
        )
    }

    /// The first `n` rows.
    pub fn head(&self, n: i64) -> DataFrame {
        DataFrame(self.0.head(Some(n as usize)))
    }

    /// `(rows, columns)`.
    pub fn shape(&self) -> (i64, i64) {
        let (r, c) = self.0.shape();
        (r as i64, c as i64)
    }

    /// The column names.
    pub fn columns(&self) -> Vec<String> {
        self.0
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Pull a column out as a typed `Vec` (the element type is inferred from the
    /// `As Vec<T>` you assign it to).
    pub fn column<T: FromColumn>(&self, name: &str) -> Vec<T> {
        let c = self.0.column(name).expect("no such column");
        T::from_column(c)
    }

    /// Write the frame to a CSV file.
    pub fn write_csv(&self, path: &str) {
        let mut df = self.0.clone();
        let mut file = std::fs::File::create(path).expect("could not create the file");
        CsvWriter::new(&mut file)
            .finish(&mut df)
            .expect("could not write the CSV file");
    }

    /// Pretty-print the frame (for debugging).
    pub fn print(&self) {
        println!("{}", self.0);
    }
}

/// Extract a polars column into a typed Rust `Vec`. Implemented for the element
/// types a VBR `Vec<T>` can hold. Assumes the column has no null values.
pub trait FromColumn {
    fn from_column(c: &Column) -> Vec<Self>
    where
        Self: Sized;
}

impl FromColumn for f64 {
    fn from_column(c: &Column) -> Vec<f64> {
        c.f64().expect("column is not a Double").into_no_null_iter().collect()
    }
}

impl FromColumn for i64 {
    fn from_column(c: &Column) -> Vec<i64> {
        c.i64().expect("column is not a Long").into_no_null_iter().collect()
    }
}

impl FromColumn for String {
    fn from_column(c: &Column) -> Vec<String> {
        c.str()
            .expect("column is not a String")
            .into_no_null_iter()
            .map(|s| s.to_string())
            .collect()
    }
}
