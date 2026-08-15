// Errors propagate automatically. Intercept a call with Handle; fail with RaiseError.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

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
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

Result_longlong_str divide(long long numerator, long long denominator);
Result_longlong_str doublequotient(long long a, long long b);

int main(void) {
    long long value = 0;
    Result_longlong_str _t0 = divide(10, 2);
    if (!_t0.is_ok) {
        char* message = _t0.err;
        printf("%s\n", vbr_concat("error: ", message));
        return 0;
    } else {
        value = _t0.ok;
    }
    printf("%s\n", vbr_concat("10 / 2 = ", vbr_from_ll(value)));
    long long bad = 0;
    Result_longlong_str _t1 = divide(7, 0);
    if (!_t1.is_ok) {
        char* message = _t1.err;
        printf("%s\n", vbr_concat("error: ", message));
        return 0;
    } else {
        bad = _t1.ok;
    }
    printf("%s\n", vbr_concat("7 / 0 = ", vbr_from_ll(bad)));
    // Failure from Divide flows out of DoubleQuotient with no extra syntax
    long long doubled = 0;
    Result_longlong_str _t2 = doublequotient(20, 4);
    if (!_t2.is_ok) {
        char* message = _t2.err;
        printf("%s\n", vbr_concat("error: ", message));
        return 0;
    } else {
        doubled = _t2.ok;
    }
    printf("%s\n", vbr_concat("double of 20 / 4 = ", vbr_from_ll(doubled)));
    Result_longlong_str _t3 = divide(9, 3);
    if (!_t3.is_ok) { fprintf(stderr, "Error: %s\n", _t3.err); return 1; }
    long long known = _t3.ok;
    printf("%s\n", vbr_concat("9 / 3 = ", vbr_from_ll(known)));
    return 0;
}

Result_longlong_str divide(long long numerator, long long denominator) {
    if ((denominator == 0)) {
        return (Result_longlong_str){ .is_ok = false, .err = vbr_dup("cannot divide by zero") };
    }
    return (Result_longlong_str){ .is_ok = true, .ok = (numerator / denominator) };
}

Result_longlong_str doublequotient(long long a, long long b) {
    Result_longlong_str _t4 = divide(a, b);
    if (!_t4.is_ok) return (Result_longlong_str){ .is_ok = false, .err = _t4.err };
    long long q = _t4.ok;
    return (Result_longlong_str){ .is_ok = true, .ok = (q * 2) };
}
