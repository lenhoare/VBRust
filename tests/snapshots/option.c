// Option<T> for maybe-absent values — Some / None

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_some; long long value; } Option_longlong;
static long long Option_longlong_unwrap(Option_longlong o) { if (!o.is_some) { fprintf(stderr, "unwrapped a None\n"); exit(1); } return o.value; }

typedef struct { bool is_ok; Option_longlong ok; char* err; } Result_opt_longlong_str;
static Option_longlong Result_opt_longlong_str_unwrap(Result_opt_longlong_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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

Result_opt_longlong_str halve(long long n);

int main(void) {
    Result_opt_longlong_str _t0 = halve(10);
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    Option_longlong _m0 = _t0.ok;
    if (_m0.is_some) {
        long long value = _m0.value;
        printf("%s\n", vbr_concat("half of 10 = ", vbr_from_ll(value)));
    } else {
        printf("%s\n", "10 is odd, no exact half");
    }
    Result_opt_longlong_str _t1 = halve(7);
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    Option_longlong _m1 = _t1.ok;
    if (_m1.is_some) {
        long long value = _m1.value;
        printf("%s\n", vbr_concat("half of 7 = ", vbr_from_ll(value)));
    } else {
        printf("%s\n", "7 is odd, no exact half");
    }
    return 0;
}

Result_opt_longlong_str halve(long long n) {
    if (((n % 2) == 0)) {
        return (Result_opt_longlong_str){ .is_ok = true, .ok = (Option_longlong){ .is_some = true, .value = (n / 2) } };
        // `/` floats; the Option<Long> payload narrows back to Long
    }
    return (Result_opt_longlong_str){ .is_ok = true, .ok = (Option_longlong){ .is_some = false } };
}
