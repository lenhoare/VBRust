// Inline list literals — `[a, b, …]` builds a Vec<T>.
// 
// Prefix `[…]` is a list; postfix `x[i]` is still indexing — no clash, exactly
// like Rust. String elements are owned automatically; numbers take their type
// from the target.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

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

typedef struct { char** data; size_t len, cap; } Vec_str;
static void Vec_str_push(Vec_str* v, char* x) {
    if (v->len == v->cap) { v->cap = v->cap ? v->cap * 2 : 4; v->data = realloc(v->data, v->cap * sizeof(char*)); }
    v->data[v->len++] = x;
}
static Vec_str Vec_str_of(size_t count, char** items) {
    Vec_str v = {0};
    for (size_t i = 0; i < count; i++) Vec_str_push(&v, items[i]);
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

long long total(Vec_longlong xs);

long long total(Vec_longlong xs) {
    long long sum = 0;
    for (size_t _i0 = 0; _i0 < xs.len; _i0++) {
        long long x = xs.data[_i0];
        sum = (sum + x);
    }
    return sum;
}

int main(void) {
    Vec_str names = Vec_str_of(3, (char*[]){ "alice", "bob", "carol" });
    printf("%s\n", vbr_concat(vbr_concat(vbr_concat("first = ", names.data[0]), ", of "), vbr_from_ll(names.len)));
    // A list literal passed straight into a function (the common case for, e.g.,
    // query parameters).
    printf("%s\n", vbr_concat("total = ", vbr_from_ll(total(Vec_longlong_of(3, (long long[]){ 10, 20, 30 })))));
    Vec_str empty = {0};
    printf("%s\n", vbr_concat("empty count = ", vbr_from_ll(empty.len)));
    return 0;
}
