// Functions, parameters and returns

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

Result_longlong_str add(long long x, long long y);
Result_longlong_str square(long long n);
Result_longlong_str factorial(long long n);

int main(void) {
    Result_longlong_str _t0 = add(2, 3);
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    long long a = _t0.ok;
    Result_longlong_str _t1 = square(4);
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    long long s = _t1.ok;
    Result_longlong_str _t2 = factorial(5);
    if (!_t2.is_ok) { fprintf(stderr, "Error: %s\n", _t2.err); return 1; }
    long long f = _t2.ok;
    printf("%s\n", vbr_concat("2 + 3 = ", vbr_from_ll(a)));
    printf("%s\n", vbr_concat("4 squared = ", vbr_from_ll(s)));
    printf("%s\n", vbr_concat("5! = ", vbr_from_ll(f)));
    return 0;
}

Result_longlong_str add(long long x, long long y) {
    return (Result_longlong_str){ .is_ok = true, .ok = (x + y) };
}

Result_longlong_str square(long long n) {
    return (Result_longlong_str){ .is_ok = true, .ok = (n * n) };
    // VB style: assign to the function name
}

Result_longlong_str factorial(long long n) {
    if ((n <= 1)) {
        return (Result_longlong_str){ .is_ok = true, .ok = 1 };
    }
    Result_longlong_str _t3 = factorial((n - 1));
    if (!_t3.is_ok) return (Result_longlong_str){ .is_ok = false, .err = _t3.err };
    return (Result_longlong_str){ .is_ok = true, .ok = (n * _t3.ok) };
}
