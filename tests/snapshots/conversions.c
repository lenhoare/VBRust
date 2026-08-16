// VB's two ways to turn text into a number, and how they differ.
// 
// Val   — the *lenient* one. Always a Double, ignores surrounding spaces,
// and returns 0 for text that isn't a number. It never fails, so
// there is nothing to handle.
// CDbl / CLng / CInt — the *strict* conversions. On text that isn't a
// number they fail. The error propagates automatically, or you
// intercept it with Handle. Use these when bad input is an error
// you want to catch, not silently turn into 0.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_ok; double ok; char* err; } Result_double_str;
static double Result_double_str_unwrap(Result_double_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; long long ok; char* err; } Result_longlong_str;
static long long Result_longlong_str_unwrap(Result_longlong_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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
static char* vbr_from_double(double d) {
    char buf[64];
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, "%.*g", p, d);
        if (strtod(buf, NULL) == d) break;
    }
    return vbr_dup(buf);
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}
static double vbr_val(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    if (!*s) return 0.0;
    char* end;
    double d = strtod(s, &end);
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (end == s || *end) return 0.0;
    return d;
}
static Result_double_str vbr_cdbl(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    if (!*s) return (Result_double_str){ .is_ok = false, .err = vbr_dup("cannot parse float from empty string") };
    char* end;
    double d = strtod(s, &end);
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (end == s || *end) return (Result_double_str){ .is_ok = false, .err = vbr_dup("invalid float literal") };
    return (Result_double_str){ .is_ok = true, .ok = d };
}
static Result_longlong_str vbr_clng(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    if (!*s) return (Result_longlong_str){ .is_ok = false, .err = vbr_dup("cannot parse integer from empty string") };
    char* end;
    long long n = strtoll(s, &end, 10);
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (end == s || *end) return (Result_longlong_str){ .is_ok = false, .err = vbr_dup("invalid digit found in string") };
    return (Result_longlong_str){ .is_ok = true, .ok = n };
}

Result_double_str priceof(char* txt);

Result_double_str priceof(char* txt) {
    // A bad CDbl fails this function — the caller Handle's it, or Main exits.
    Result_double_str _t0 = vbr_cdbl(txt);
    if (!_t0.is_ok) return (Result_double_str){ .is_ok = false, .err = _t0.err };
    return (Result_double_str){ .is_ok = true, .ok = _t0.ok };
}

int main(void) {
    // Lenient: 0 on nonsense, spaces ignored, always a Double.
    printf("%s\n", vbr_from_double(vbr_val("3.14")));
    printf("%s\n", vbr_from_double(vbr_val("  42  ")));
    printf("%s\n", vbr_from_double(vbr_val("nonsense")));
    // A Double flows into a Long with Bust's automatic numeric cast.
    long long count = vbr_val("100");
    printf("%s\n", vbr_from_ll(count));
    // Strict: intercept failure with Handle.
    long long v = 0;
    Result_longlong_str _t1 = vbr_clng("77");
    if (!_t1.is_ok) {
        char* e = _t1.err;
        printf("%s\n", vbr_concat("not a number: ", e));
        return 0;
    } else {
        v = _t1.ok;
    }
    printf("%s\n", vbr_concat("parsed ", vbr_from_ll(v)));
    double p = 0;
    Result_double_str _t2 = priceof("9.99");
    if (!_t2.is_ok) {
        char* e = _t2.err;
        printf("%s\n", e);
        return 0;
    } else {
        p = _t2.ok;
    }
    printf("%s\n", vbr_concat("price is ", vbr_from_double(p)));
    return 0;
}
