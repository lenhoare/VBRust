# Native dataframes — read a CSV, compute new columns with Excel-style array
# formulas, filter rows, select, and pull a column out. Backed by polars (pure
# Rust — no Python). See dataframe_spec.md.

from vbrpy import _vb, col, lit, read_csv, when

def main():
    df: object = read_csv('people.csv')
    rows, cols = df.shape
    print(f"loaded {_vb(rows)} rows, {_vb(cols)} columns")
    # Column formulas: arithmetic across whole columns, and an IIf band.
    df = df.with_columns((col("price") * col("qty")).alias('total'))
    df = df.with_columns(when((col("age") >= lit(18))).then(lit('adult')).otherwise(lit('minor')).alias('band'))
    # Row filter: a boolean mask over columns, combined with a Dim'd value.
    cutoff: int = 30
    df = df.filter(((col("age") > lit(cutoff)) & col("active")))
    df = df.select(['name', 'band', 'total'])
    print(df)
    names: list[str] = df['name'].to_list()
    print(f"first kept: {_vb(names[0])}")
    df.write_csv('out.csv')
    print('wrote out.csv')


if __name__ == "__main__":
    main()
