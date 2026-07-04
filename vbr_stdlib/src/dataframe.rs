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

    /// Group rows by one or more key columns. Finish with `.agg(...)` — the
    /// pair reads like SQL's `GROUP BY … SELECT agg(…)`:
    /// `df.group_by(&["band"]).agg(&[col("age").mean()])`.
    pub fn group_by(&self, keys: &[&str]) -> GroupedFrame {
        GroupedFrame {
            df: self.0.clone(),
            keys: keys.iter().map(|k| k.to_string()).collect(),
        }
    }

    /// The sum of a numeric column, as a `Double`.
    pub fn sum(&self, name: &str) -> f64 {
        self.scalar_agg(col(name).sum(), "Sum")
    }

    /// The mean (average) of a numeric column.
    pub fn mean(&self, name: &str) -> f64 {
        self.scalar_agg(col(name).mean(), "Mean")
    }

    /// The smallest value in a numeric column.
    pub fn min(&self, name: &str) -> f64 {
        self.scalar_agg(col(name).min(), "Min")
    }

    /// The largest value in a numeric column.
    pub fn max(&self, name: &str) -> f64 {
        self.scalar_agg(col(name).max(), "Max")
    }

    /// One whole-column aggregation, cast to `f64` (VBR's `Double`) so every
    /// scalar aggregation comes back as the same simple number type.
    fn scalar_agg(&self, e: Expr, what: &str) -> f64 {
        let out = self
            .0
            .clone()
            .lazy()
            .select([e.cast(DataType::Float64).alias("agg")])
            .collect()
            .unwrap_or_else(|_| panic!("{} failed — is the column numeric?", what));
        out.column("agg")
            .expect("aggregation column")
            .f64()
            .expect("aggregation value")
            .get(0)
            .unwrap_or(f64::NAN)
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

/// The intermediate of `group_by(…)` — rows grouped by key columns, waiting
/// for `.agg(…)` to say what to compute per group. Grouping runs through
/// polars' *stable* group-by, so groups keep first-seen order (deterministic
/// output, which matters for a teaching tool).
pub struct GroupedFrame {
    df: polars::prelude::DataFrame,
    keys: Vec<String>,
}

impl GroupedFrame {
    /// Aggregate each group: one expression per output column, e.g.
    /// `col("age").mean()`, `(col("price") * col("qty")).sum()`.
    pub fn agg(&self, exprs: &[Expr]) -> DataFrame {
        let keys: Vec<Expr> = self.keys.iter().map(|k| col(k.as_str())).collect();
        DataFrame(
            self.df
                .clone()
                .lazy()
                .group_by_stable(keys)
                .agg(exprs.to_vec())
                .collect()
                .expect("group/agg failed"),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DataFrame {
        DataFrame(
            polars::df!(
                "band" => ["a", "b", "a", "b", "a"],
                "v" => [1i64, 2, 3, 4, 5],
            )
            .unwrap(),
        )
    }

    #[test]
    fn scalar_aggregations() {
        let d = sample();
        assert_eq!(d.sum("v"), 15.0);
        assert_eq!(d.mean("v"), 3.0);
        assert_eq!(d.min("v"), 1.0);
        assert_eq!(d.max("v"), 5.0);
    }

    #[test]
    fn group_by_agg() {
        let d = sample();
        let g = d.group_by(&["band"]).agg(&[col("v").sum()]);
        assert_eq!(g.shape(), (2, 2));
        // Stable grouping: "a" was seen first.
        assert_eq!(g.column::<String>("band"), vec!["a", "b"]);
        assert_eq!(g.column::<i64>("v"), vec![9, 6]);
    }

    #[test]
    fn group_by_formula_agg() {
        let d = sample();
        // An expression inside the aggregation: sum of v*2 per group.
        let g = d
            .group_by(&["band"])
            .agg(&[(col("v") * lit(2)).sum().alias("v2")]);
        assert_eq!(g.column::<i64>("v2"), vec![18, 12]);
    }
}
