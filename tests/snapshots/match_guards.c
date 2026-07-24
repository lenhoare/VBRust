// Guards (`If`) and the `_` wildcard. A guard is a Rust match guard — the arm
// only fires when its condition is also true. `x` binds the matched value.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

char* describe(long long n);

char* describe(long long n) {
    long long _m0 = n;
    if (_m0 == 0) {
        return "zero";
    } else if ((_m0 < 0)) {
        return "negative";
    } else if ((_m0 > 100)) {
        return "huge";
    } else {
        return "ordinary";
    }
}

int main(void) {
    printf("%s\n", vbr_concat("-3 is ", describe(-3)));
    printf("%s\n", vbr_concat("0 is ", describe(0)));
    printf("%s\n", vbr_concat("42 is ", describe(42)));
    printf("%s\n", vbr_concat("500 is ", describe(500)));
    return 0;
}
