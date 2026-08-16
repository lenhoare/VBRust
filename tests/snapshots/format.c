// Format uses a Rust format string, not VB's #.### pictures.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}
static char* vbr_fmt_double(double x, const char* spec) {
    char b[128];
    snprintf(b, sizeof b, spec, x);
    return vbr_dup(b);
}
static char* vbr_fmt_ll(long long x, const char* spec) {
    char b[128];
    snprintf(b, sizeof b, spec, x);
    return vbr_dup(b);
}

int main(void) {
    printf("%s\n", vbr_fmt_double(3.14159, "%.2f"));
    printf("%s\n", vbr_fmt_ll(7, "%04lld"));
    printf("%s\n", vbr_concat("approx ", vbr_fmt_double(3.5, "%.1f")));
    return 0;
}
