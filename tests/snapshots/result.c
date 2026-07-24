// Result<T> for fallible functions — propagate with ?, handle with Match

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_ok; long long ok; char* err; } Result_longlong_str;
static long long Result_longlong_str_unwrap(Result_longlong_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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
    // Handle the outcome explicitly
    Result_longlong_str _m0 = divide(10, 2);
    if (_m0.is_ok) {
        long long value = _m0.ok;
        printf("%s\n", vbr_concat("10 / 2 = ", vbr_from_ll(value)));
    } else {
        char* message = _m0.err;
        printf("%s\n", vbr_concat("error: ", message));
    }
    Result_longlong_str _m1 = divide(7, 0);
    if (_m1.is_ok) {
        long long value = _m1.ok;
        printf("%s\n", vbr_concat("7 / 0 = ", vbr_from_ll(value)));
    } else {
        char* message = _m1.err;
        printf("%s\n", vbr_concat("error: ", message));
    }
    // A function that uses ? to propagate failure
    Result_longlong_str _m2 = doublequotient(20, 4);
    if (_m2.is_ok) {
        long long value = _m2.ok;
        printf("%s\n", vbr_concat("double of 20 / 4 = ", vbr_from_ll(value)));
    } else {
        char* message = _m2.err;
        printf("%s\n", vbr_concat("error: ", message));
    }
    // .Unwrap() is allowed, but training wheels
    long long known = Result_longlong_str_unwrap(divide(9, 3));
    printf("%s\n", vbr_concat("9 / 3 = ", vbr_from_ll(known)));
    return 0;
}

Result_longlong_str divide(long long numerator, long long denominator) {
    if ((denominator == 0)) {
        return (Result_longlong_str){ .is_ok = false, .err = "cannot divide by zero" };
    }
    return (Result_longlong_str){ .is_ok = true, .ok = (numerator / denominator) };
}

Result_longlong_str doublequotient(long long a, long long b) {
    Result_longlong_str _t0 = divide(a, b);
    if (!_t0.is_ok) return (Result_longlong_str){ .is_ok = false, .err = _t0.err };
    long long q = _t0.ok;
    return (Result_longlong_str){ .is_ok = true, .ok = (q * 2) };
}
