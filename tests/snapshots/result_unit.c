// A fallible action that returns no value on success. RaiseError fails; a
// bare End Function is success. Intercept at the call with Handle.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_ok; char* err; } Result_unit_str;
static void Result_unit_str_unwrap(Result_unit_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } }

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

Result_unit_str save(bool ok);

Result_unit_str save(bool ok) {
    if ((!ok)) {
        return (Result_unit_str){ .is_ok = false, .err = vbr_dup("save failed") };
    }
    return (Result_unit_str){ .is_ok = true };
}

int main(void) {
    Result_unit_str _t0 = save(true);
    if (!_t0.is_ok) {
        char* e = _t0.err;
        printf("%s\n", vbr_concat("error: ", e));
        return 0;
    }
    printf("%s\n", "saved");
    Result_unit_str _t1 = save(false);
    if (!_t1.is_ok) {
        char* e = _t1.err;
        printf("%s\n", vbr_concat("error: ", e));
        return 0;
    }
    printf("%s\n", "saved");
    return 0;
}
