# GroupBy and aggregation — group rows by a key column and compute per-group
# summaries with the same Excel-style formulas, plus whole-column scalar
# aggregations (a single Double out). See dataframe_spec.md §4b.

from vbrpy import _vb, col, lit, read_csv, when

def main():
    df: object = read_csv('people.csv')
    df = df.with_columns(when((col("age") >= lit(18))).then(lit('adult')).otherwise(lit('minor')).alias('band'))
    # Whole-column scalars — one number for the whole frame.
    print(f"mean age: {_vb(df['age'].mean())}")
    print(f"max age:  {_vb(df['age'].max())}")
    # Per-group aggregation: one row per band, one column per aggregation.
    byband: object = df.group_by(['band']).agg([col("name").count(), col("age").mean(), col("qty").sum()])
    print(byband)
    # Formulas work inside an aggregation too.
    spend: object = df.group_by(['band']).agg([(col("price") * col("qty")).sum()])
    print(spend)


if __name__ == "__main__":
    main()
