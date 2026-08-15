// Typed errors are strings on the implicit Result<T, String> channel.
// RaiseError fails; Handle intercepts a call and binds the message.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_ok; int ok; char* err; } Result_int_str;
static int Result_int_str_unwrap(Result_int_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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

Result_int_str safediv(int a, int b);
Result_int_str doublediv(int a, int b);

Result_int_str safediv(int a, int b) {
    if ((b == 0)) {
        return (Result_int_str){ .is_ok = false, .err = vbr_dup("div by zero") };
    }
    if ((b < 0)) {
        return (Result_int_str){ .is_ok = false, .err = vbr_dup("negative divisor") };
    }
    return (Result_int_str){ .is_ok = true, .ok = (a / b) };
}

Result_int_str doublediv(int a, int b) {
    Result_int_str _t0 = safediv(a, b);
    if (!_t0.is_ok) return (Result_int_str){ .is_ok = false, .err = _t0.err };
    int q = _t0.ok;
    return (Result_int_str){ .is_ok = true, .ok = (q * 2) };
}

int main(void) {
    int v = 0;
    Result_int_str _t1 = doublediv(10, 2);
    if (!_t1.is_ok) {
        char* _ = _t1.err;
        printf("%s\n", "failed");
        return 0;
    } else {
        v = _t1.ok;
    }
    printf("%s\n", vbr_concat("ok: ", vbr_from_ll(v)));
    int ignored = 0;
    Result_int_str _t2 = doublediv(10, 0);
    if (!_t2.is_ok) {
        char* _ = _t2.err;
        printf("%s\n", "failed");
        return 0;
    } else {
        ignored = _t2.ok;
    }
    printf("%s\n", vbr_concat("ok: ", vbr_from_ll(ignored)));
    int v3 = 0;
    Result_int_str _t3 = doublediv(10, -2);
    if (!_t3.is_ok) {
        char* err = _t3.err;
        printf("%s\n", vbr_concat("error: ", err));
        return 0;
    } else {
        v3 = _t3.ok;
    }
    printf("%s\n", vbr_concat("ok: ", vbr_from_ll(v3)));
    return 0;
}
