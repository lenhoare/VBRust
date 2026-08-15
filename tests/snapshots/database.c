// database.vbr — SQLite via the stdlib. A Database is a live connection you
// hold and call methods on (like Json/DateTime, not a stateless namespace).
// 
// Params bind to ? placeholders (always as text — column affinity stores them
// typed, so declare columns INTEGER/REAL). NULL goes in the SQL itself:
// VALUES (?, NULL) — a list of strings has no null slot. Query rows come back
// as Json objects keyed by column name, each column with its natural type.
// A ByVal Database param borrows the connection (&Database) — open once,
// hand it around. Fallible calls propagate automatically.
// text and score are `&str` params. Dropping them straight into the params list
// fills a `Vec<String>`, so each is owned with `.to_string()` for you — no manual
// `.clone()` or `CStr(...)`. A literal element (none here) is owned by the list
// emitter as before.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <sqlite3.h>
#include "cJSON.h"

typedef struct { cJSON *node; } Json;

typedef struct { sqlite3 *conn; } Database;

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

typedef struct { bool is_ok; char* err; } Result_unit_str;
static void Result_unit_str_unwrap(Result_unit_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } }

typedef struct { bool is_ok; long long ok; char* err; } Result_longlong_str;
static long long Result_longlong_str_unwrap(Result_longlong_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { long long* data; size_t len, cap; } Vec_longlong;
static void Vec_longlong_push(Vec_longlong* v, long long x) {
    if (v->len == v->cap) { v->cap = v->cap ? v->cap * 2 : 4; v->data = realloc(v->data, v->cap * sizeof(long long)); }
    v->data[v->len++] = x;
}
static Vec_longlong Vec_longlong_of(size_t count, long long* items) {
    Vec_longlong v = {0};
    for (size_t i = 0; i < count; i++) Vec_longlong_push(&v, items[i]);
    return v;
}

typedef struct { char** data; size_t len, cap; } Vec_str;
static void Vec_str_push(Vec_str* v, char* x) {
    if (v->len == v->cap) { v->cap = v->cap ? v->cap * 2 : 4; v->data = realloc(v->data, v->cap * sizeof(char*)); }
    v->data[v->len++] = x;
}
static Vec_str Vec_str_of(size_t count, char** items) {
    Vec_str v = {0};
    for (size_t i = 0; i < count; i++) Vec_str_push(&v, items[i]);
    return v;
}

typedef struct { bool is_ok; Vec_Json ok; char* err; } Result_vec_Json_str;
static Vec_Json Result_vec_Json_str_unwrap(Result_vec_Json_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; double ok; char* err; } Result_double_str;
static double Result_double_str_unwrap(Result_double_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; Json ok; char* err; } Result_Json_str;
static Json Result_Json_str_unwrap(Result_Json_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; Database ok; char* err; } Result_Database_str;
static Database Result_Database_str_unwrap(Result_Database_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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
static char* vbr_from_double(double d) {
    char buf[64];
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, "%.*g", p, d);
        if (strtod(buf, NULL) == d) break;
    }
    return vbr_dup(buf);
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
static Result_Database_str vbr_db_open(char* path) {
    sqlite3* conn;
    if (sqlite3_open(path, &conn) != SQLITE_OK) {
        char* e = vbr_dup(sqlite3_errmsg(conn));
        sqlite3_close(conn);
        return (Result_Database_str){ .is_ok = false, .err = e };
    }
    return (Result_Database_str){ .is_ok = true, .ok = (Database){ .conn = conn } };
}
static void vbr_db__bind(sqlite3_stmt* stmt, Vec_str params) {
    for (size_t i = 0; i < params.len; i++)
        sqlite3_bind_text(stmt, (int)i + 1, params.data[i], -1, SQLITE_TRANSIENT);
}
static Result_longlong_str vbr_db_execute(Database* self, char* sql, Vec_str params) {
    sqlite3_stmt* stmt;
    if (sqlite3_prepare_v2(self->conn, sql, -1, &stmt, NULL) != SQLITE_OK)
        return (Result_longlong_str){ .is_ok = false, .err = vbr_dup(sqlite3_errmsg(self->conn)) };
    vbr_db__bind(stmt, params);
    int rc = sqlite3_step(stmt);
    if (rc != SQLITE_DONE && rc != SQLITE_ROW) {
        char* e = vbr_dup(sqlite3_errmsg(self->conn));
        sqlite3_finalize(stmt);
        return (Result_longlong_str){ .is_ok = false, .err = e };
    }
    sqlite3_finalize(stmt);
    return (Result_longlong_str){ .is_ok = true, .ok = sqlite3_changes(self->conn) };
}
static Result_vec_Json_str vbr_db_query(Database* self, char* sql, Vec_str params) {
    sqlite3_stmt* stmt;
    if (sqlite3_prepare_v2(self->conn, sql, -1, &stmt, NULL) != SQLITE_OK)
        return (Result_vec_Json_str){ .is_ok = false, .err = vbr_dup(sqlite3_errmsg(self->conn)) };
    vbr_db__bind(stmt, params);
    Vec_Json out = {0};
    int ncol = sqlite3_column_count(stmt);
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        cJSON* obj = cJSON_CreateObject();
        for (int i = 0; i < ncol; i++) {
            const char* name = sqlite3_column_name(stmt, i);
            switch (sqlite3_column_type(stmt, i)) {
                case SQLITE_INTEGER: cJSON_AddNumberToObject(obj, name, (double)sqlite3_column_int64(stmt, i)); break;
                case SQLITE_FLOAT:   cJSON_AddNumberToObject(obj, name, sqlite3_column_double(stmt, i)); break;
                case SQLITE_TEXT:    cJSON_AddStringToObject(obj, name, (const char*)sqlite3_column_text(stmt, i)); break;
                default:             cJSON_AddNullToObject(obj, name); break;
            }
        }
        Vec_Json_push(&out, (Json){ .node = obj });
    }
    sqlite3_finalize(stmt);
    return (Result_vec_Json_str){ .is_ok = true, .ok = out };
}
static long long vbr_db_lastinsertid(Database* self) {
    return sqlite3_last_insert_rowid(self->conn);
}

Result_unit_str run(Database db);
Result_unit_str addscored(Database db, char* text, char* score);

Result_unit_str run(Database db) {
    Result_longlong_str _t0 = vbr_db_execute(&db, "CREATE TABLE IF NOT EXISTS ideas (id INTEGER PRIMARY KEY, gen INTEGER, text TEXT, score REAL, parent INTEGER)", (Vec_str){0});
    if (!_t0.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t0.err };
    _t0.ok;
    Result_longlong_str _t1 = vbr_db_execute(&db, "DELETE FROM ideas", (Vec_str){0});
    if (!_t1.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t1.err };
    _t1.ok;
    // A root idea has no parent — the NULL is written in the SQL.
    Result_longlong_str _t2 = vbr_db_execute(&db, "INSERT INTO ideas (gen, text, score, parent) VALUES (1, ?, ?, NULL)", Vec_str_of(2, (char*[]){ "solar tracker", "0.82" }));
    if (!_t2.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t2.err };
    _t2.ok;
    long long root = vbr_db_lastinsertid(&db);
    // A child links to its parent via the fresh rowid — lineage.
    Result_longlong_str _t3 = vbr_db_execute(&db, "INSERT INTO ideas (gen, text, score, parent) VALUES (2, ?, ?, ?)", Vec_str_of(3, (char*[]){ "improved tracker", "0.91", vbr_from_ll(root) }));
    if (!_t3.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t3.err };
    _t3.ok;
    // Insert through a helper whose text/score arrive as ByVal String params —
    // a `&str` element in the params list, owned into the Vec<String> for you.
    Result_unit_str _t4 = addscored(db, "wind turbine", "0.75");
    if (!_t4.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t4.err };
    (void)0;
    Result_vec_Json_str _t5 = vbr_db_query(&db, "SELECT text, score, parent FROM ideas ORDER BY score DESC", (Vec_str){0});
    if (!_t5.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t5.err };
    Vec_Json rows = _t5.ok;
    for (size_t _i6 = 0; _i6 < rows.len; _i6++) {
        Json row = rows.data[_i6];
        Result_str_str _t7 = vbr_json_getstring(&row, "text");
        if (!_t7.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t7.err };
        Result_double_str _t8 = vbr_json_getfloat(&row, "score");
        if (!_t8.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t8.err };
        char* line = vbr_concat(vbr_concat(_t7.ok, " scores "), vbr_from_double(_t8.ok));
        Result_Json_str _t9 = vbr_json_get(&row, "parent");
        if (!_t9.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t9.err };
        if (vbr_json_isnull(&_t9.ok)) {
            printf("%s\n", vbr_concat(line, " (a root idea)"));
        } else {
            Result_longlong_str _t10 = vbr_json_getint(&row, "parent");
            if (!_t10.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t10.err };
            printf("%s\n", vbr_concat(vbr_concat(vbr_concat(line, " (child of #"), vbr_from_ll(_t10.ok)), ")"));
        }
    }
    return (Result_unit_str){ .is_ok = true };
}

Result_unit_str addscored(Database db, char* text, char* score) {
    Result_longlong_str _t11 = vbr_db_execute(&db, "INSERT INTO ideas (gen, text, score, parent) VALUES (3, ?, ?, NULL)", Vec_str_of(2, (char*[]){ text, score }));
    if (!_t11.is_ok) return (Result_unit_str){ .is_ok = false, .err = _t11.err };
    _t11.ok;
    return (Result_unit_str){ .is_ok = true };
}

int main(void) {
    Result_Database_str _t12 = vbr_db_open("ideas.db");
    if (!_t12.is_ok) { fprintf(stderr, "Error: %s\n", _t12.err); return 1; }
    Database db = _t12.ok;
    Result_unit_str _t13 = run(db);
    if (!_t13.is_ok) { fprintf(stderr, "Error: %s\n", _t13.err); return 1; }
    (void)0;
    printf("%s\n", "done");
    return 0;
}
