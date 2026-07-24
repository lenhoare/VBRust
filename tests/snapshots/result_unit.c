// Result<()> — a fallible action that returns no value on success. `Ok(())` is
// the unit success; failure carries the error as usual.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { bool is_ok; char* err; } Result_unit_str;
static void Result_unit_str_unwrap(Result_unit_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } }

static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

Result_unit_str save(bool ok);

Result_unit_str save(bool ok) {
    if ((!ok)) {
        return (Result_unit_str){ .is_ok = false, .err = "save failed" };
    }
    return (Result_unit_str){ .is_ok = true };
}

int main(void) {
    Result_unit_str _m0 = save(true);
    if (_m0.is_ok) {
        printf("%s\n", "saved");
    } else {
        char* e = _m0.err;
        printf("%s\n", vbr_concat("error: ", e));
    }
    Result_unit_str _m1 = save(false);
    if (_m1.is_ok) {
        printf("%s\n", "saved");
    } else {
        char* e = _m1.err;
        printf("%s\n", vbr_concat("error: ", e));
    }
    return 0;
}
