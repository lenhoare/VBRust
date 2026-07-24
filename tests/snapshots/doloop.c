// Do loops, Exit and Continue

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

int main(void) {
    long long i = 1;
    while ((i <= 3)) {
        printf("%s\n", vbr_concat("while ", vbr_from_ll(i)));
        i = (i + 1);
    }
    long long j = 10;
    while (!((j == 0))) {
        j = (j - 2);
    }
    printf("%s\n", vbr_concat("j ended at ", vbr_from_ll(j)));
    long long n = 0;
    do {
        n = (n + 1);
    } while ((n < 3));
    printf("%s\n", vbr_concat("n = ", vbr_from_ll(n)));
    for (long long k = 1; k <= 6; k++) {
        if ((k == 4)) {
            break;
        }
        if ((k == 2)) {
            continue;
        }
        printf("%s\n", vbr_concat("k = ", vbr_from_ll(k)));
    }
    return 0;
}
