// Simple enums — a named set of variants. They're Copy, compare with `=`, and
// pair naturally with Match. Reference a variant as `Suit.Hearts` → `Suit::Hearts`.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef enum { Suit_Hearts, Suit_Diamonds, Suit_Clubs, Suit_Spades } Suit;

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

Result_str_str color(Suit s);

Result_str_str color(Suit s) {
    Suit _m0 = s;
    if (_m0 == Suit_Hearts) {
        return (Result_str_str){ .is_ok = true, .ok = "red" };
    } else if (_m0 == Suit_Diamonds) {
        return (Result_str_str){ .is_ok = true, .ok = "red" };
    } else if (_m0 == Suit_Clubs) {
        return (Result_str_str){ .is_ok = true, .ok = "black" };
    } else {
        return (Result_str_str){ .is_ok = true, .ok = "black" };
    }
    return (Result_str_str){ .is_ok = true, .ok = 0 };
}

int main(void) {
    Suit s = Suit_Spades;
    Result_str_str _t0 = color(s);
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    printf("%s\n", vbr_concat("Spades are ", _t0.ok));
    Result_str_str _t1 = color(Suit_Hearts);
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    printf("%s\n", vbr_concat("Hearts are ", _t1.ok));
    if ((s == Suit_Spades)) {
        printf("%s\n", "yes, spades");
    }
    return 0;
}
