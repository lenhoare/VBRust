// Built-in maths functions (work on floating-point values)

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <math.h>

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

int main(void) {
    double x = 9.0;
    double angle = 0.0;
    printf("%s\n", vbr_concat("sqrt(9)    = ", vbr_from_double(sqrt(x))));
    printf("%s\n", vbr_concat("abs(-5)    = ", vbr_from_double(fabs(-5.0))));
    printf("%s\n", vbr_concat("9 ^ 2      = ", vbr_from_double(pow(x, 2))));
    printf("%s\n", vbr_concat("9 ^ 0.5    = ", vbr_from_double(pow(x, 0.5))));
    printf("%s\n", vbr_concat("int(3.7)   = ", vbr_from_double(floor(3.7))));
    printf("%s\n", vbr_concat("round(3.5) = ", vbr_from_double(round(3.5))));
    printf("%s\n", vbr_concat("sin(0)     = ", vbr_from_double(sin(angle))));
    printf("%s\n", vbr_concat("cos(0)     = ", vbr_from_double(cos(angle))));
    printf("%s\n", vbr_concat("tan(0)     = ", vbr_from_double(tan(angle))));
    printf("%s\n", vbr_concat("atn(0)     = ", vbr_from_double(atan(angle))));
    printf("%s\n", vbr_concat("exp(1)     = ", vbr_from_double(exp(1.0))));
    printf("%s\n", vbr_concat("ln(e)      = ", vbr_from_double(log(2.718281828))));
    // Mod gives the remainder (→ Rust's %, same precedence as * and /)
    long long n = 17;
    printf("%s\n", vbr_concat("17 Mod 5   = ", vbr_from_ll((n % 5))));
    return 0;
}
