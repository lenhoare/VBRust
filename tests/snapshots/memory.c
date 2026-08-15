// Manual memory, made visible. A heap string is released early with `x = Nothing`
// — VB6's object-release idiom, carried over as Bust's explicit "I'm done with
// this" hook. It matters most on the C target, where nothing is freed for you:
// • C      → free(greeting); greeting = NULL;
// • Rust   → drop(greeting);   (the compiler usually inserts this at scope end)
// • Python → greeting = None   (the garbage collector reclaims it)
// The program's output is identical on all three targets.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}

int main(void) {
    char* greeting = vbr_dup("hello, world");
    printf("%s\n", greeting);
    // Done with it — release it now rather than waiting for scope end.
    free(greeting);
    greeting = NULL;
    printf("%s\n", "released the greeting");
    return 0;
}
