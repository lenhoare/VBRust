// Vec<T> — a growable list

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

int main(void) {
    Vec_longlong nums = {0};
    Vec_longlong_push(&nums, 10);
    Vec_longlong_push(&nums, 20);
    Vec_longlong_push(&nums, 30);
    printf("%s\n", vbr_concat("count = ", vbr_from_ll(nums.len)));
    long long total = 0;
    for (size_t _i0 = 0; _i0 < nums.len; _i0++) {
        long long n = nums.data[_i0];
        total = (total + n);
    }
    printf("%s\n", vbr_concat("total = ", vbr_from_ll(total)));
    return 0;
}
