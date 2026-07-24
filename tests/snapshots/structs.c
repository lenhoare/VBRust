// Structs — Type/End Type, construction, and field access

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct {
    char* name;
    long long age;
} Person;

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
    Person alice = (Person){ .name = "Alice", .age = 30 };
    printf("%s\n", vbr_concat(vbr_concat(alice.name, " is "), vbr_from_ll(alice.age)));
    alice.age = (alice.age + 1);
    printf("%s\n", vbr_concat(vbr_concat(vbr_concat("after a birthday, ", alice.name), " is "), vbr_from_ll(alice.age)));
    return 0;
}
