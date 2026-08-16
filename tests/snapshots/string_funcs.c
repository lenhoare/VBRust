// Built-in string functions

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

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
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}
static int vbr_utf8_clen(unsigned char c) {
    if (c < 0x80) return 1;
    if ((c & 0xE0) == 0xC0) return 2;
    if ((c & 0xF0) == 0xE0) return 3;
    if ((c & 0xF8) == 0xF0) return 4;
    return 1;
}
static long long vbr_len(const char* s) {
    long long n = 0;
    for (const unsigned char* p = (const unsigned char*)s; *p; ) {
        p += vbr_utf8_clen(*p);
        n++;
    }
    return n;
}
static char* vbr_left(const char* s, long long n) {
    if (n <= 0) return vbr_dup("");
    const unsigned char* p = (const unsigned char*)s;
    long long i = 0;
    while (*p && i < n) { p += vbr_utf8_clen(*p); i++; }
    size_t nbytes = (size_t)(p - (const unsigned char*)s);
    char* d = (char*)malloc(nbytes + 1);
    memcpy(d, s, nbytes);
    d[nbytes] = 0;
    return d;
}
static char* vbr_right(const char* s, long long n) {
    if (n <= 0) return vbr_dup("");
    long long len = vbr_len(s);
    long long skip = len > n ? len - n : 0;
    const unsigned char* p = (const unsigned char*)s;
    long long i = 0;
    while (*p && i < skip) { p += vbr_utf8_clen(*p); i++; }
    return vbr_dup((const char*)p);
}
static char* vbr_mid(const char* s, long long start) {
    if (start < 1) start = 1;
    const unsigned char* p = (const unsigned char*)s;
    long long i = 1;
    while (*p && i < start) { p += vbr_utf8_clen(*p); i++; }
    return vbr_dup((const char*)p);
}
static char* vbr_mid_n(const char* s, long long start, long long count) {
    if (count <= 0) return vbr_dup("");
    if (start < 1) start = 1;
    const unsigned char* p = (const unsigned char*)s;
    long long i = 1;
    while (*p && i < start) { p += vbr_utf8_clen(*p); i++; }
    const unsigned char* b = p;
    long long k = 0;
    while (*p && k < count) { p += vbr_utf8_clen(*p); k++; }
    size_t nbytes = (size_t)(p - b);
    char* d = (char*)malloc(nbytes + 1);
    memcpy(d, b, nbytes);
    d[nbytes] = 0;
    return d;
}
static char* vbr_ucase(const char* s) {
    char* d = vbr_dup(s);
    for (char* p = d; *p; p++) if (*p >= 'a' && *p <= 'z') *p = (char)(*p - 32);
    return d;
}
static char* vbr_lcase(const char* s) {
    char* d = vbr_dup(s);
    for (char* p = d; *p; p++) if (*p >= 'A' && *p <= 'Z') *p = (char)(*p + 32);
    return d;
}
static char* vbr_trim(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    const char* e = s + strlen(s);
    while (e > s && (e[-1] == ' ' || e[-1] == '\t' || e[-1] == '\n' || e[-1] == '\r')) e--;
    size_t n = (size_t)(e - s);
    char* d = (char*)malloc(n + 1);
    memcpy(d, s, n);
    d[n] = 0;
    return d;
}
static char* vbr_replace(const char* s, const char* a, const char* b) {
    size_t al = strlen(a), bl = strlen(b);
    if (al == 0) return vbr_dup(s);
    size_t count = 0;
    for (const char* p = s; (p = strstr(p, a)); p += al) count++;
    char* out = (char*)malloc(strlen(s) + count * bl + 1);
    char* o = out;
    const char* p = s;
    const char* hit;
    while ((hit = strstr(p, a))) {
        size_t n = (size_t)(hit - p);
        memcpy(o, p, n); o += n;
        memcpy(o, b, bl); o += bl;
        p = hit + al;
    }
    strcpy(o, p);
    return out;
}
static char* vbr_chr(long long n) {
    unsigned char c = (unsigned char)n;
    char* d = (char*)malloc(5);
    if (c < 0x80) { d[0] = (char)c; d[1] = 0; }
    else {
        d[0] = (char)(0xC0 | (c >> 6));
        d[1] = (char)(0x80 | (c & 0x3F));
        d[2] = 0;
    }
    return d;
}
static long long vbr_asc(const char* s) {
    if (!s || !*s) return 0;
    const unsigned char* p = (const unsigned char*)s;
    unsigned char c = p[0];
    if (c < 0x80) return (long long)c;
    if ((c & 0xE0) == 0xC0 && p[1])
        return (long long)(((c & 0x1F) << 6) | (p[1] & 0x3F));
    if ((c & 0xF0) == 0xE0 && p[1] && p[2])
        return (long long)(((c & 0x0F) << 12) | ((p[1] & 0x3F) << 6) | (p[2] & 0x3F));
    if ((c & 0xF8) == 0xF0 && p[1] && p[2] && p[3])
        return (long long)(((c & 0x07) << 18) | ((p[1] & 0x3F) << 12) | ((p[2] & 0x3F) << 6) | (p[3] & 0x3F));
    return (long long)c;
}

int main(void) {
    char* s = vbr_dup("Hello, World");
    printf("%s\n", vbr_concat("length:    ", vbr_from_ll(vbr_len(s))));
    printf("%s\n", vbr_concat("upper:     ", vbr_ucase(s)));
    printf("%s\n", vbr_concat("lower:     ", vbr_lcase(s)));
    printf("%s\n", vbr_concat("left 5:    ", vbr_left(s, 5)));
    printf("%s\n", vbr_concat("right 5:   ", vbr_right(s, 5)));
    printf("%s\n", vbr_concat("mid 2,3:   ", vbr_mid_n(s, 2, 3)));
    printf("%s\n", vbr_concat("trimmed:   ", vbr_trim("   padded   ")));
    printf("%s\n", vbr_concat("replaced:  ", vbr_replace(s, "World", "Rust")));
    printf("%s\n", vbr_concat("str of 42: ", vbr_from_ll(42)));
    return 0;
}
