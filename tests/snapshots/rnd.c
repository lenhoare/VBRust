// Rnd() is a Double in [0, 1). A die is Int(Rnd() * 6) + 1.
// The printed line is a range check so the snapshot stays deterministic.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <time.h>

static double rnd(void) {
    static int seeded = 0;
    if (!seeded) {
        srand((unsigned)time(NULL));
        seeded = 1;
    }
    return (double)rand() / ((double)RAND_MAX + 1.0);
}

int main(void) {
    double r = rnd();
    if (((r >= 0.0) && (r < 1.0))) {
        printf("%s\n", "ok");
    }
    return 0;
}
