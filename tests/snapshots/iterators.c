// Iterators — filter, map, sum, any, count, collect

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

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
static char* vbr_from_ll(long long v) {
    char* s = (char*)malloc(32);
    snprintf(s, 32, "%lld", v);
    return s;
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
    Vec_longlong nums = {0};
    Vec_longlong_push(&nums, 1);
    Vec_longlong_push(&nums, 2);
    Vec_longlong_push(&nums, 3);
    Vec_longlong_push(&nums, 4);
    Vec_longlong_push(&nums, 5);
    Vec_longlong big = {0};
    for (size_t _i0 = 0; _i0 < nums.len; _i0++) {
        long long x = nums.data[_i0];
        if ((x > 2)) Vec_longlong_push(&big, x);
    }
    Vec_longlong doubled = {0};
    for (size_t _i1 = 0; _i1 < nums.len; _i1++) {
        long long x = nums.data[_i1];
        Vec_longlong_push(&doubled, (x * 2));
    }
    long long total = 0;
    for (size_t _i2 = 0; _i2 < nums.len; _i2++) {
        total += nums.data[_i2];
    }
    bool hasbig = false;
    for (size_t _i3 = 0; _i3 < nums.len; _i3++) {
        long long x = nums.data[_i3];
        if ((x > 4)) { hasbig = true; break; }
    }
    printf("%s\n", vbr_concat("count:   ", vbr_from_ll(nums.len)));
    printf("%s\n", vbr_concat("total:   ", vbr_from_ll(total)));
    printf("%s\n", vbr_concat("has big: ", vbr_from_bool(hasbig)));
    for (size_t _i4 = 0; _i4 < big.len; _i4++) {
        long long n = big.data[_i4];
        printf("%s\n", vbr_concat("big:     ", vbr_from_ll(n)));
    }
    for (size_t _i5 = 0; _i5 < doubled.len; _i5++) {
        long long n = doubled.data[_i5];
        printf("%s\n", vbr_concat("doubled: ", vbr_from_ll(n)));
    }
    return 0;
}
