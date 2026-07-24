// Simple enums — a named set of variants. They're Copy, compare with `=`, and
// pair naturally with Match. Reference a variant as `Suit.Hearts` → `Suit::Hearts`.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef enum { Suit_Hearts, Suit_Diamonds, Suit_Clubs, Suit_Spades } Suit;

static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

char* color(Suit s);

char* color(Suit s) {
    Suit _m0 = s;
    if (_m0 == Suit_Hearts) {
        return "red";
    } else if (_m0 == Suit_Diamonds) {
        return "red";
    } else if (_m0 == Suit_Clubs) {
        return "black";
    } else {
        return "black";
    }
}

int main(void) {
    Suit s = Suit_Spades;
    printf("%s\n", vbr_concat("Spades are ", color(s)));
    printf("%s\n", vbr_concat("Hearts are ", color(Suit_Hearts)));
    if ((s == Suit_Spades)) {
        printf("%s\n", "yes, spades");
    }
    return 0;
}
