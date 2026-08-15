// Struct methods — impl, Me/self, and &self vs &mut self

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct {
    char* name;
    long long age;
} Person;

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; char* err; } Result_unit_str;
static void Result_unit_str_unwrap(Result_unit_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } }

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

Result_str_str Person_greet(Person* self);
Result_unit_str Person_havebirthday(Person* self);

Result_str_str Person_greet(Person* self) {
    return (Result_str_str){ .is_ok = true, .ok = vbr_concat(vbr_concat(vbr_concat(vbr_concat("Hi, I'm ", self->name), " ("), vbr_from_ll(self->age)), ")") };
}

Result_unit_str Person_havebirthday(Person* self) {
    self->age = (self->age + 1);
    return (Result_unit_str){ .is_ok = true };
}

int main(void) {
    Person alice = (Person){ .name = "Alice", .age = 30 };
    Result_str_str _t0 = Person_greet(&alice);
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    printf("%s\n", _t0.ok);
    Result_unit_str _t1 = Person_havebirthday(&alice);
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    (void)0;
    Result_str_str _t2 = Person_greet(&alice);
    if (!_t2.is_ok) { fprintf(stderr, "Error: %s\n", _t2.err); return 1; }
    printf("%s\n", _t2.ok);
    return 0;
}
