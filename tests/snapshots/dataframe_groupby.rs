// GroupBy and aggregation — group rows by a key column and compute per-group
// summaries with the same Excel-style formulas, plus whole-column scalar
// aggregations (a single Double out). See dataframe_spec.md §4b.

use vbr_stdlib::{DataFrame};

use vbr_stdlib::dataframe::{col, lit, when};

fn main() {
    let mut df: DataFrame = DataFrame::read_csv("people.csv");
    df = df.with_column("band", when(col("age").gt_eq(lit(18))).then(lit("adult")).otherwise(lit("minor")));
    // Whole-column scalars — one number for the whole frame.
    println!("mean age: {}", df.mean("age"));
    println!("max age:  {}", df.max("age"));
    // Per-group aggregation: one row per band, one column per aggregation.
    let byband: DataFrame = df.group_by(&["band"]).agg(&[col("name").count(), col("age").mean(), col("qty").sum()]);
    byband.print();
    // Formulas work inside an aggregation too.
    let spend: DataFrame = df.group_by(&["band"]).agg(&[(col("price") * col("qty")).sum()]);
    spend.print();
}
