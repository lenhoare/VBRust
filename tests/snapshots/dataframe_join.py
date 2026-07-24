# Joins — combine two frames on a shared key column, SQL-style: Join (inner),
# LeftJoin (keep all left rows), OuterJoin (keep everything). Where a key has
# no match the new cells are null — Filter with IsNull before extracting.

from vbrpy import _vb, col, read_csv

def main():
    people: object = read_csv('people.csv')
    orders: object = read_csv('orders.csv')
    # Inner join: only the people who placed orders.
    buyers: object = people.join(orders, on='name', how='inner')
    print(buyers)
    # Left join: everyone; item/amount are null where nobody bought.
    everyone: object = people.join(orders, on='name', how='left')
    print(everyone)
    # Nulls have no VBR type — filter unmatched rows out, then extract.
    matched: object = everyone.filter((~col("item").is_null()))
    amounts: list[float] = matched['amount'].to_list()
    total: float = 0
    for a in amounts:
        total += a
    print(f"spent: {_vb(total)}")


if __name__ == "__main__":
    main()
