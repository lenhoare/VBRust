// Result<T, E> with a real, typed error enum — including a message-carrying
// variant. Build errors with Err(MathError.…); read them back by matching. `?`
// works when the error types line up.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef enum { MathError_DivByZero, MathError_Custom } MathErrorTag;
typedef struct {
    MathErrorTag tag;
    union {
        struct { char* f0; } Custom;
    } data;
} MathError;

typedef struct { bool is_ok; int ok; MathError err; } Result_int_MathError;
static int Result_int_MathError_unwrap(Result_int_MathError r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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

Result_int_MathError safediv(int a, int b);
Result_int_MathError doublediv(int a, int b);

Result_int_MathError safediv(int a, int b) {
    if ((b == 0)) {
        return (Result_int_MathError){ .is_ok = false, .err = (MathError){ .tag = MathError_DivByZero } };
    }
    if ((b < 0)) {
        return (Result_int_MathError){ .is_ok = false, .err = (MathError){ .tag = MathError_Custom, .data.Custom = { "negative divisor" } } };
    }
    return (Result_int_MathError){ .is_ok = true, .ok = (a / b) };
}

Result_int_MathError doublediv(int a, int b) {
    Result_int_MathError _t0 = safediv(a, b);
    if (!_t0.is_ok) return (Result_int_MathError){ .is_ok = false, .err = _t0.err };
    int q = _t0.ok;
    return (Result_int_MathError){ .is_ok = true, .ok = (q * 2) };
}

int main(void) {
    Result_int_MathError _m0 = doublediv(10, 2);
    if (_m0.is_ok) {
        int v = _m0.ok;
        printf("%s\n", vbr_concat("ok: ", vbr_from_ll(v)));
    } else if (!_m0.is_ok && _m0.err.tag == MathError_DivByZero) {
        printf("%s\n", "div by zero");
    } else {
        char* msg = _m0.err.data.Custom.f0;
        printf("%s\n", vbr_concat("error: ", msg));
    }
    Result_int_MathError _m1 = doublediv(10, 0);
    if (_m1.is_ok) {
        int v = _m1.ok;
        printf("%s\n", vbr_concat("ok: ", vbr_from_ll(v)));
    } else {
        printf("%s\n", "failed");
    }
    Result_int_MathError _m2 = doublediv(10, -2);
    if (_m2.is_ok) {
        int v = _m2.ok;
        printf("%s\n", vbr_concat("ok: ", vbr_from_ll(v)));
    } else if (!_m2.is_ok && _m2.err.tag == MathError_DivByZero) {
        printf("%s\n", "div by zero");
    } else {
        char* msg = _m2.err.data.Custom.f0;
        printf("%s\n", vbr_concat("error: ", msg));
    }
    return 0;
}
