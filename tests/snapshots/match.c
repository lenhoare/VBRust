// Match → Rust's `match`. Each arm is `pattern => body`; the patterns are real
// Rust — literals, ranges (`..=`), alternation (`|`), and the `_` wildcard.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

int main(void) {
    long long score = 75;
    long long _m0 = score;
    if (_m0 == 100) {
        printf("%s\n", "perfect");
    } else if (_m0 >= 90 && _m0 <= 99) {
        printf("%s\n", "excellent");
    } else if (_m0 >= 70 && _m0 <= 89) {
        printf("%s\n", "good");
    } else if (((_m0 == 0) || (_m0 == 1) || (_m0 == 2))) {
        printf("%s\n", "very low");
    } else {
        printf("%s\n", "somewhere in between");
    }
    return 0;
}
