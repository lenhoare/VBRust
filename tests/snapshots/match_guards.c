// Guards (`If`) and the `_` wildcard. A guard is a Rust match guard — the arm
// only fires when its condition is also true. `x` binds the matched value.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

Result_str_str describe(long long n);

Result_str_str describe(long long n) {
    long long _m0 = n;
    if (_m0 == 0) {
        return (Result_str_str){ .is_ok = true, .ok = "zero" };
    } else if ((_m0 < 0)) {
        return (Result_str_str){ .is_ok = true, .ok = "negative" };
    } else if ((_m0 > 100)) {
        return (Result_str_str){ .is_ok = true, .ok = "huge" };
    } else {
        return (Result_str_str){ .is_ok = true, .ok = "ordinary" };
    }
    return (Result_str_str){ .is_ok = true, .ok = 0 };
}

int main(void) {
    Result_str_str _t0 = describe(-3);
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    printf("%s\n", vbr_concat("-3 is ", _t0.ok));
    Result_str_str _t1 = describe(0);
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    printf("%s\n", vbr_concat("0 is ", _t1.ok));
    Result_str_str _t2 = describe(42);
    if (!_t2.is_ok) { fprintf(stderr, "Error: %s\n", _t2.err); return 1; }
    printf("%s\n", vbr_concat("42 is ", _t2.ok));
    Result_str_str _t3 = describe(500);
    if (!_t3.is_ok) { fprintf(stderr, "Error: %s\n", _t3.err); return 1; }
    printf("%s\n", vbr_concat("500 is ", _t3.ok));
    return 0;
}
