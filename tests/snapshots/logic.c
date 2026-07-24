// Logical operators: And, Or, Not, Xor. Logical (short-circuit) and looser
// than comparison, just like Rust's &&, ||, !, ^ — no backwards-compat quirks.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
static char* vbr_from_bool(bool b) {
    return vbr_dup(b ? "true" : "false");
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

int main(void) {
    long long age = 30;
    bool member = true;
    if (((age >= 18) && member)) {
        printf("%s\n", "admitted");
    }
    if (((age < 13) || (age > 65))) {
        printf("%s\n", "discounted");
    } else {
        printf("%s\n", "full price");
    }
    if ((!member)) {
        printf("%s\n", "please join");
    } else {
        printf("%s\n", "welcome back");
    }
    // Xor: true when exactly one side is true.
    bool heads = true;
    bool tails = false;
    printf("%s\n", vbr_concat("valid coin: ", vbr_from_bool((heads != tails))));
    // Precedence: And binds tighter than Or, comparisons tighter than both.
    bool ok = (((age > 0) && (age < 120)) || member);
    printf("%s\n", vbr_concat("ok: ", vbr_from_bool(ok)));
    return 0;
}
