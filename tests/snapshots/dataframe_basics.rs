// Native dataframes — read a CSV, compute new columns with Excel-style array
// formulas, filter rows, select, and pull a column out. Backed by polars (pure
// Rust — no Python). See dataframe_spec.md.

use vbr_stdlib::{DataFrame};

#[allow(unused_imports)]
use vbr_stdlib::dataframe::{col, lit, when};

fn main() {
    let mut df: DataFrame = DataFrame::read_csv("people.csv");
    let (rows, cols): (i64, i64) = df.shape();
    println!("loaded {} rows, {} columns", rows, cols);
    // Column formulas: arithmetic across whole columns, and an IIf band.
    df = df.with_column("total", col("price") * col("qty"));
    df = df.with_column("band", when(col("age").gt_eq(lit(18))).then(lit("adult")).otherwise(lit("minor")));
    // Row filter: a boolean mask over columns, combined with a Dim'd value.
    let cutoff: i64 = 30;
    df = df.filter(col("age").gt(lit(cutoff)).and(col("active")));
    df = df.select(&["name", "band", "total"]);
    df.print();
    let names: Vec<String> = df.column("name");
    println!("first kept: {}", names[0]);
    df.write_csv("out.csv");
    println!("wrote out.csv");
}
