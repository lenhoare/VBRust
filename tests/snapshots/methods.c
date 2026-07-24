// Struct methods — impl, Me/self, and &self vs &mut self

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

char* Person_greet(Person* self);
void Person_havebirthday(Person* self);

char* Person_greet(Person* self) {
    return vbr_concat(vbr_concat(vbr_concat(vbr_concat("Hi, I'm ", self->name), " ("), vbr_from_ll(self->age)), ")");
}

void Person_havebirthday(Person* self) {
    self->age = (self->age + 1);
}

int main(void) {
    Person alice = (Person){ .name = "Alice", .age = 30 };
    printf("%s\n", Person_greet(&alice));
    Person_havebirthday(&alice);
    printf("%s\n", Person_greet(&alice));
    return 0;
}
