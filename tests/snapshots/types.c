// Every fixed-size type — these all copy freely

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

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
static char* vbr_from_bool(bool b) {
    return vbr_dup(b ? "true" : "false");
}
static char* vbr_from_double(double d) {
    char buf[64];
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, "%.*g", p, d);
        if (strtod(buf, NULL) == d) break;
    }
    return vbr_dup(buf);
}
static char* vbr_from_float(float f) {
    char buf[32];
    for (int p = 1; p <= 9; p++) {
        snprintf(buf, sizeof buf, "%.*g", p, (double)f);
        if (strtof(buf, NULL) == f) break;
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
    int small = 42;
    long long count = 100000;
    long long huge = 9000000000;
    float pi = 3.14;
    double ratio = 2.5;
    bool flag = true;
    unsigned char letter = 65;
    printf("%s\n", vbr_concat("small  = ", vbr_from_ll(small)));
    printf("%s\n", vbr_concat("count  = ", vbr_from_ll(count)));
    printf("%s\n", vbr_concat("huge   = ", vbr_from_ll(huge)));
    printf("%s\n", vbr_concat("pi     = ", vbr_from_float(pi)));
    printf("%s\n", vbr_concat("ratio  = ", vbr_from_double(ratio)));
    printf("%s\n", vbr_concat("flag   = ", vbr_from_bool(flag)));
    printf("%s\n", vbr_concat("letter = ", vbr_from_ll(letter)));
    return 0;
}
