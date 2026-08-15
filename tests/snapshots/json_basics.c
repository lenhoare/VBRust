// Json from the standard library — parse a document and read typed fields.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include "cJSON.h"

typedef struct { cJSON *node; } Json;

typedef struct { Json* data; size_t len, cap; } Vec_Json;
static void Vec_Json_push(Vec_Json* v, Json x) {
    if (v->len == v->cap) { v->cap = v->cap ? v->cap * 2 : 4; v->data = realloc(v->data, v->cap * sizeof(Json)); }
    v->data[v->len++] = x;
}
static Vec_Json Vec_Json_of(size_t count, Json* items) {
    Vec_Json v = {0};
    for (size_t i = 0; i < count; i++) Vec_Json_push(&v, items[i]);
    return v;
}

typedef struct { bool is_ok; Json ok; char* err; } Result_Json_str;
static Json Result_Json_str_unwrap(Result_Json_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; long long ok; char* err; } Result_longlong_str;
static long long Result_longlong_str_unwrap(Result_longlong_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; Vec_Json ok; char* err; } Result_vec_Json_str;
static Vec_Json Result_vec_Json_str_unwrap(Result_vec_Json_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; double ok; char* err; } Result_double_str;
static double Result_double_str_unwrap(Result_double_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; bool ok; char* err; } Result_bool_str;
static bool Result_bool_str_unwrap(Result_bool_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

static char* vbr_json__err(const char* pre, const char* key, const char* post) {
    char* s = (char*)malloc(strlen(pre) + strlen(key) + strlen(post) + 1);
    strcpy(s, pre); strcat(s, key); strcat(s, post);
    return s;
}
static cJSON* vbr_json__field(Json* self, char* key) {
    return cJSON_GetObjectItemCaseSensitive(self->node, key);
}
static Result_Json_str vbr_json_parse(char* text) {
    cJSON* n = cJSON_Parse(text);
    if (!n) return (Result_Json_str){ .is_ok = false, .err = vbr_dup("invalid JSON") };
    return (Result_Json_str){ .is_ok = true, .ok = (Json){ .node = n } };
}
static Json vbr_json_object(void) { return (Json){ .node = cJSON_CreateObject() }; }
static Json vbr_json_array(void) { return (Json){ .node = cJSON_CreateArray() }; }
static bool vbr_json_haskey(Json* self, char* key) { return vbr_json__field(self, key) != NULL; }
static bool vbr_json_isnull(Json* self) { return cJSON_IsNull(self->node); }
static Result_str_str vbr_json_getstring(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_str_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' not found") };
    if (!cJSON_IsString(f)) return (Result_str_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' is not a string") };
    return (Result_str_str){ .is_ok = true, .ok = vbr_dup(f->valuestring) };
}
static Result_longlong_str vbr_json_getint(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_longlong_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' not found") };
    if (!cJSON_IsNumber(f) || f->valuedouble != (double)(long long)f->valuedouble)
        return (Result_longlong_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' is not an integer") };
    return (Result_longlong_str){ .is_ok = true, .ok = (long long)f->valuedouble };
}
static Result_double_str vbr_json_getfloat(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_double_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' not found") };
    if (!cJSON_IsNumber(f)) return (Result_double_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' is not a float") };
    return (Result_double_str){ .is_ok = true, .ok = f->valuedouble };
}
static Result_bool_str vbr_json_getbool(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_bool_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' not found") };
    if (!cJSON_IsBool(f)) return (Result_bool_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' is not a boolean") };
    return (Result_bool_str){ .is_ok = true, .ok = cJSON_IsTrue(f) };
}
static Result_vec_Json_str vbr_json_getarray(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_vec_Json_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' not found") };
    if (!cJSON_IsArray(f)) return (Result_vec_Json_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' is not an array") };
    Vec_Json out = {0};
    cJSON* it = NULL;
    cJSON_ArrayForEach(it, f) Vec_Json_push(&out, (Json){ .node = it });
    return (Result_vec_Json_str){ .is_ok = true, .ok = out };
}
static Result_Json_str vbr_json_get(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_Json_str){ .is_ok = false, .err = vbr_json__err("Key '", key, "' not found") };
    return (Result_Json_str){ .is_ok = true, .ok = (Json){ .node = f } };
}
static Result_str_str vbr_json_asstring(Json* self) {
    if (!cJSON_IsString(self->node)) return (Result_str_str){ .is_ok = false, .err = vbr_dup("value is not a string") };
    return (Result_str_str){ .is_ok = true, .ok = vbr_dup(self->node->valuestring) };
}
static Result_longlong_str vbr_json_asint(Json* self) {
    if (!cJSON_IsNumber(self->node) || self->node->valuedouble != (double)(long long)self->node->valuedouble)
        return (Result_longlong_str){ .is_ok = false, .err = vbr_dup("value is not an integer") };
    return (Result_longlong_str){ .is_ok = true, .ok = (long long)self->node->valuedouble };
}
static Result_double_str vbr_json_asfloat(Json* self) {
    if (!cJSON_IsNumber(self->node)) return (Result_double_str){ .is_ok = false, .err = vbr_dup("value is not a float") };
    return (Result_double_str){ .is_ok = true, .ok = self->node->valuedouble };
}
static Result_bool_str vbr_json_asbool(Json* self) {
    if (!cJSON_IsBool(self->node)) return (Result_bool_str){ .is_ok = false, .err = vbr_dup("value is not a boolean") };
    return (Result_bool_str){ .is_ok = true, .ok = cJSON_IsTrue(self->node) };
}
static Result_str_str vbr_json_tostring(Json* self) {
    char* s = cJSON_PrintUnformatted(self->node);
    if (!s) return (Result_str_str){ .is_ok = false, .err = vbr_dup("could not serialise") };
    char* d = vbr_dup(s); free(s);
    return (Result_str_str){ .is_ok = true, .ok = d };
}
static Result_str_str vbr_json_topretty(Json* self) {
    char* s = cJSON_Print(self->node);
    if (!s) return (Result_str_str){ .is_ok = false, .err = vbr_dup("could not serialise") };
    char* d = vbr_dup(s); free(s);
    return (Result_str_str){ .is_ok = true, .ok = d };
}
static void vbr_json_setstring(Json* self, char* key, char* val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddStringToObject(self->node, key, val);
}
static void vbr_json_setint(Json* self, char* key, long long val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddNumberToObject(self->node, key, (double)val);
}
static void vbr_json_setbool(Json* self, char* key, bool val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddBoolToObject(self->node, key, val);
}
static void vbr_json_set(Json* self, char* key, Json val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddItemToObject(self->node, key, val.node);
}
static void vbr_json_push(Json* self, Json val) {
    cJSON_AddItemToArray(self->node, val.node);
}

int main(void) {
    Result_Json_str _t0 = vbr_json_parse("{\"name\":\"Alice\",\"age\":42}");
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    Json person = _t0.ok;
    Result_str_str _t1 = vbr_json_getstring(&person, "name");
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    printf("%s\n", vbr_concat("name = ", _t1.ok));
    Result_longlong_str _t2 = vbr_json_getint(&person, "age");
    if (!_t2.is_ok) { fprintf(stderr, "Error: %s\n", _t2.err); return 1; }
    printf("%s\n", vbr_concat("age  = ", vbr_from_ll(_t2.ok)));
    Result_Json_str _t3 = vbr_json_parse("{\"tags\":[\"red\",\"green\",\"blue\"]}");
    if (!_t3.is_ok) { fprintf(stderr, "Error: %s\n", _t3.err); return 1; }
    Json doc = _t3.ok;
    Result_vec_Json_str _t4 = vbr_json_getarray(&doc, "tags");
    if (!_t4.is_ok) { fprintf(stderr, "Error: %s\n", _t4.err); return 1; }
    Vec_Json tags = _t4.ok;
    printf("%s\n", vbr_concat("tag count: ", vbr_from_ll(tags.len)));
    for (size_t _i5 = 0; _i5 < tags.len; _i5++) {
        Json tag = tags.data[_i5];
        Result_str_str _t6 = vbr_json_asstring(&tag);
        if (!_t6.is_ok) { fprintf(stderr, "Error: %s\n", _t6.err); return 1; }
        printf("%s\n", vbr_concat("  tag: ", _t6.ok));
    }
    return 0;
}
