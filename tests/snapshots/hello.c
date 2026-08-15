// Bust vertical-slice demo — everything here is in the first milestone

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
    long long count = 3;
    long long total = 0;
    for (long long i = 1; i <= count; i++) {
        total = (total + i);
    }
    double ratio = 2.5;
    printf("%s\n", vbr_concat(vbr_concat(vbr_concat("Sum 1..", vbr_from_ll(count)), " = "), vbr_from_ll(total)));
    printf("%s\n", vbr_concat("ratio is ", vbr_from_double(ratio)));
    if ((total > 5)) {
        printf("%s\n", "big");
    } else if ((total == 5)) {
        printf("%s\n", "exactly five");
    } else {
        printf("%s\n", "small");
    }
    return 0;
}
