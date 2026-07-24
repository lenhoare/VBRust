// Constants — module level, SCREAMING_SNAKE_CASE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

static const long long MAX_RETRIES = 3;
static const char* GREETING = "Hello";
static const double VERSION = 1.5;

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
    long long i = 0;
    while ((i < MAX_RETRIES)) {
        printf("%s\n", vbr_concat(vbr_concat(GREETING, ", attempt "), vbr_from_ll((i + 1))));
        i = (i + 1);
    }
    printf("%s\n", vbr_concat("version ", vbr_from_double(VERSION)));
    return 0;
}
