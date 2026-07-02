// Native dataframes — read a CSV, compute new columns with Excel-style array
// formulas, filter rows, select, and pull a column out. Backed by polars (pure
// Rust — no Python). See dataframe_spec.md.

use vbr_stdlib::{DataFrame};

use vbr_stdlib::dataframe::{col, lit, when};

fn main() {
    let mut df: DataFrame = DataFrame::readcsv("people.csv");
    let (rows, cols): (i64, i64) = df.shape();
    println!("loaded {} rows, {} columns", rows, cols);
    // Column formulas: arithmetic across whole columns, and an IIf band.
    df = df.withcolumn("total", price * qty);
    df = df.withcolumn("band", iif(age >= 18, "adult", "minor"));
    // Row filter: a boolean mask over columns, combined with a Dim'd value.
    let cutoff: i64 = 30;
    df = df.filter(col("age").gt(lit(cutoff)).and(col("active")));
    df = df.select(&["name", "band", "total"]);
    df.print();
    let names: Vec<String> = df.column("name");
    println!("first kept: {}", names[0]);
    df.writecsv("out.csv");
    println!("wrote out.csv");
}
