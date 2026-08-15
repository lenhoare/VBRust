// HashMap<K, V> — VBA's Scripting.Dictionary, done natively

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { char* key; long long val; } Map_str_longlongEntry;
typedef struct { Map_str_longlongEntry* entries; size_t len, cap; } Map_str_longlong;
static void Map_str_longlong_insert(Map_str_longlong* m, char* k, long long val) {
    for (size_t i = 0; i < m->len; i++) if (strcmp(m->entries[i].key, k) == 0) { m->entries[i].val = val; return; }
    if (m->len == m->cap) { m->cap = m->cap ? m->cap * 2 : 4; m->entries = realloc(m->entries, m->cap * sizeof(Map_str_longlongEntry)); }
    m->entries[m->len].key = k; m->entries[m->len].val = val; m->len++;
}
static long long* Map_str_longlong_get(Map_str_longlong* m, char* k) {
    for (size_t i = 0; i < m->len; i++) if (strcmp(m->entries[i].key, k) == 0) return &m->entries[i].val;
    return NULL;
}
static bool Map_str_longlong_contains(Map_str_longlong* m, char* k) { return Map_str_longlong_get(m, k) != NULL; }

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
static char* vbr_from_ll(long long v) {
    char* s = (char*)malloc(32);
    snprintf(s, 32, "%lld", v);
    return s;
}
static char* vbr_from_bool(bool b) {
    return vbr_dup(b ? "true" : "false");
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

int main(void) {
    Map_str_longlong ages = {0};
    Map_str_longlong_insert(&ages, "Alice", 30);
    Map_str_longlong_insert(&ages, "Bob", 25);
    printf("%s\n", vbr_concat("has Alice? ", vbr_from_bool(Map_str_longlong_contains(&ages, "Alice"))));
    printf("%s\n", vbr_concat("has Bob?   ", vbr_from_bool(Map_str_longlong_contains(&ages, "Bob"))));
    printf("%s\n", vbr_concat("has Carol? ", vbr_from_bool(Map_str_longlong_contains(&ages, "Carol"))));
    for (size_t _i0 = 0; _i0 < ages.len; _i0++) {
        char* name = ages.entries[_i0].key;
        long long age = ages.entries[_i0].val;
        printf("%s\n", vbr_concat(vbr_concat(name, " is "), vbr_from_ll(age)));
    }
    return 0;
}
