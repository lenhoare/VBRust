# Match → Rust's `match`. Each arm is `pattern => body`; the patterns are real
# Rust — literals, ranges (`..=`), alternation (`|`), and the `_` wildcard.

def main():
    score: int = 75
    _m0 = score
    match _m0:
        case 100:
            print('perfect')
        case _ if 90 <= _m0 <= 99:
            print('excellent')
        case _ if 70 <= _m0 <= 89:
            print('good')
        case 0 | 1 | 2:
            print('very low')
        case _:
            print('somewhere in between')


if __name__ == "__main__":
    main()
