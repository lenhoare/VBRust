// Functions, parameters and returns

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

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

long long add(long long x, long long y);
long long square(long long n);
long long factorial(long long n);

int main(void) {
    long long a = add(2, 3);
    long long s = square(4);
    long long f = factorial(5);
    printf("%s\n", vbr_concat("2 + 3 = ", vbr_from_ll(a)));
    printf("%s\n", vbr_concat("4 squared = ", vbr_from_ll(s)));
    printf("%s\n", vbr_concat("5! = ", vbr_from_ll(f)));
    return 0;
}

long long add(long long x, long long y) {
    return (x + y);
}

long long square(long long n) {
    return (n * n);
    // VB style: assign to the function name
}

long long factorial(long long n) {
    if ((n <= 1)) {
        return 1;
    }
    return (n * factorial((n - 1)));
}
