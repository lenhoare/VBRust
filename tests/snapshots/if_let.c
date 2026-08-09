// `If <expr> Is <pattern> Then …` — VB-flavoured `if let`. Handle just the one
// case you care about (usually Some/Ok) and skip the rest, without a full Match.
// `Is` is VB's word (as in VB6's `Is Nothing`); the Rust backend emits a real
// `if let`, so you see the idiomatic construct.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_some; long long value; } Option_longlong;
static long long Option_longlong_unwrap(Option_longlong o) { if (!o.is_some) { fprintf(stderr, "unwrapped a None\n"); exit(1); } return o.value; }

static char* vbr_from_ll(long long v) {
    char* s = (char*)malloc(32);
    snprintf(s, 32, "%lld", v);
    return s;
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

Option_longlong findprice(char* item);

Option_longlong findprice(char* item) {
    if ((item == "apple")) {
        return (Option_longlong){ .is_some = true, .value = 30 };
    }
    return (Option_longlong){ .is_some = false };
}

int main(void) {
    Option_longlong _m0 = findprice("apple");
    if (_m0.is_some) {
        long long price = _m0.value;
        printf("%s\n", vbr_concat("apple costs ", vbr_from_ll(price)));
    } else {
    }
    Option_longlong _m1 = findprice("pear");
    if (_m1.is_some) {
        long long price = _m1.value;
        printf("%s\n", vbr_concat("pear costs ", vbr_from_ll(price)));
        // never runs — pear has no price
    } else {
    }
    // Single-line form, too.
    Option_longlong _m2 = findprice("apple");
    if (_m2.is_some) {
        long long price = _m2.value;
        printf("%s\n", vbr_concat("again: ", vbr_from_ll(price)));
    } else {
    }
    return 0;
}
