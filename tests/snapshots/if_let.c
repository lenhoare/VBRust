// `If <expr> Is <pattern> Then …` — VB-flavoured `if let`. Handle just the one
// case you care about (usually Some/Ok), with an optional `Else`. `Is` is VB's
// word (as in VB6's `Is Nothing`). `Do While <expr> Is <pattern>` is `while let`:
// loop while the pattern keeps matching. The Rust backend emits the idiomatic
// `if let` / `loop { if let … else break }`.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_some; long long value; } Option_longlong;
static long long Option_longlong_unwrap(Option_longlong o) { if (!o.is_some) { fprintf(stderr, "unwrapped a None\n"); exit(1); } return o.value; }

typedef struct { long long* data; size_t len, cap; } Vec_longlong;
static void Vec_longlong_push(Vec_longlong* v, long long x) {
    if (v->len == v->cap) { v->cap = v->cap ? v->cap * 2 : 4; v->data = realloc(v->data, v->cap * sizeof(long long)); }
    v->data[v->len++] = x;
}
static Vec_longlong Vec_longlong_of(size_t count, long long* items) {
    Vec_longlong v = {0};
    for (size_t i = 0; i < count; i++) Vec_longlong_push(&v, items[i]);
    return v;
}

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
Option_longlong nextitem(Vec_longlong xs, long long idx);

Option_longlong findprice(char* item) {
    if ((item == "apple")) {
        return (Option_longlong){ .is_some = true, .value = 30 };
    }
    return (Option_longlong){ .is_some = false };
}

Option_longlong nextitem(Vec_longlong xs, long long idx) {
    if ((idx < xs.len)) {
        return (Option_longlong){ .is_some = true, .value = xs.data[idx] };
    }
    return (Option_longlong){ .is_some = false };
}

int main(void) {
    Option_longlong _m0 = findprice("apple");
    if (_m0.is_some) {
        long long price = _m0.value;
        printf("%s\n", vbr_concat("apple costs ", vbr_from_ll(price)));
    } else {
        printf("%s\n", "no price for apple");
    }
    Option_longlong _m1 = findprice("pear");
    if (_m1.is_some) {
        long long price = _m1.value;
        printf("%s\n", vbr_concat("pear costs ", vbr_from_ll(price)));
    } else {
        printf("%s\n", "no price for pear");
    }
    // Single-line form.
    Option_longlong _m2 = findprice("apple");
    if (_m2.is_some) {
        long long price = _m2.value;
        printf("%s\n", vbr_concat("again: ", vbr_from_ll(price)));
    } else {
    }
    // while let — drain a list of prices.
    Vec_longlong prices = Vec_longlong_of(3, (long long[]){ 10, 20, 30 });
    long long i = 0;
    while (1) {
        Option_longlong _m3 = nextitem(prices, i);
        if (_m3.is_some) {
            long long v = _m3.value;
            printf("%s\n", vbr_concat("item ", vbr_from_ll(v)));
            i = (i + 1);
        } else {
            break;
        }
    }
    return 0;
}
