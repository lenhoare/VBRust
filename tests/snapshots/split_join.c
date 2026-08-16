// Split, Join, Space — VB's string-list builtins.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

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

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}
static Vec_str vbr_split(const char* s, const char* delim) {
    Vec_str v = {0};
    size_t dlen = strlen(delim);
    if (dlen == 0) {
        Vec_str_push(&v, vbr_dup(s));
        return v;
    }
    const char* p = s;
    for (;;) {
        const char* hit = strstr(p, delim);
        if (!hit) {
            Vec_str_push(&v, vbr_dup(p));
            break;
        }
        size_t n = (size_t)(hit - p);
        char* part = (char*)malloc(n + 1);
        memcpy(part, p, n);
        part[n] = 0;
        Vec_str_push(&v, part);
        p = hit + dlen;
    }
    return v;
}
static char* vbr_join(Vec_str v, const char* delim) {
    if (v.len == 0) return vbr_dup("");
    size_t dlen = strlen(delim);
    size_t total = dlen * (v.len - 1);
    for (size_t i = 0; i < v.len; i++) total += strlen(v.data[i]);
    char* out = (char*)malloc(total + 1);
    char* o = out;
    for (size_t i = 0; i < v.len; i++) {
        size_t n = strlen(v.data[i]);
        memcpy(o, v.data[i], n);
        o += n;
        if (i + 1 < v.len) {
            memcpy(o, delim, dlen);
            o += dlen;
        }
    }
    *o = 0;
    return out;
}
static char* vbr_space(long long n) {
    if (n < 0) n = 0;
    char* d = (char*)malloc((size_t)n + 1);
    memset(d, ' ', (size_t)n);
    d[n] = 0;
    return d;
}

int main(void) {
    char* csv = vbr_dup("one,two,three");
    Vec_str parts = vbr_split(csv, ",");
    printf("%s\n", vbr_join(parts, " / "));
    // Default delimiter is a single space, both ways.
    printf("%s\n", vbr_join(vbr_split("a b c", " "), " "));
    printf("%s\n", vbr_concat(vbr_concat("[", vbr_space(3)), "]"));
    return 0;
}
