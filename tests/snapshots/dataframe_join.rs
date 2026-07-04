// Joins — combine two frames on a shared key column, SQL-style: Join (inner),
// LeftJoin (keep all left rows), OuterJoin (keep everything). Where a key has
// no match the new cells are null — Filter with IsNull before extracting.

use vbr_stdlib::{DataFrame};

#[allow(unused_imports)]
use vbr_stdlib::dataframe::{col, lit, when};

fn main() {
    let people: DataFrame = DataFrame::read_csv("people.csv");
    let orders: DataFrame = DataFrame::read_csv("orders.csv");
    // Inner join: only the people who placed orders.
    let buyers: DataFrame = people.join(&orders, &["name"]);
    buyers.print();
    // Left join: everyone; item/amount are null where nobody bought.
    let everyone: DataFrame = people.left_join(&orders, &["name"]);
    everyone.print();
    // Nulls have no VBR type — filter unmatched rows out, then extract.
    let matched: DataFrame = everyone.filter(col("item").is_null().not());
    let amounts: Vec<f64> = matched.column("amount");
    let mut total: f64 = 0.0;
    for a in &amounts {
        total += *a;
    }
    println!("spent: {}", total);
}
