// Option<T> for maybe-absent values — Some / None

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_some; long long value; } Option_longlong;
static long long Option_longlong_unwrap(Option_longlong o) { if (!o.is_some) { fprintf(stderr, "unwrapped a None\n"); exit(1); } return o.value; }

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

Option_longlong halve(long long n);

int main(void) {
    Option_longlong _m0 = halve(10);
    if (_m0.is_some) {
        long long value = _m0.value;
        printf("%s\n", vbr_concat("half of 10 = ", vbr_from_ll(value)));
    } else {
        printf("%s\n", "10 is odd, no exact half");
    }
    Option_longlong _m1 = halve(7);
    if (_m1.is_some) {
        long long value = _m1.value;
        printf("%s\n", vbr_concat("half of 7 = ", vbr_from_ll(value)));
    } else {
        printf("%s\n", "7 is odd, no exact half");
    }
    return 0;
}

Option_longlong halve(long long n) {
    if ((((n / 2) * 2) == n)) {
        return (Option_longlong){ .is_some = true, .value = (n / 2) };
    }
    return (Option_longlong){ .is_some = false };
}
