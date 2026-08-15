// http_post.vbr — POST a JSON body with request headers.
// 
// This is the shape of an LLM API call: a JSON body, a Content-Type, and an
// `Authorization: Bearer` token. Headers are a HashMap<String, String> (VB's
// Scripting.Dictionary); pass an empty one for no custom headers.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <curl/curl.h>

typedef struct { char* key; char* val; } Map_str_strEntry;
typedef struct { Map_str_strEntry* entries; size_t len, cap; } Map_str_str;
static void Map_str_str_insert(Map_str_str* m, char* k, char* val) {
    for (size_t i = 0; i < m->len; i++) if (strcmp(m->entries[i].key, k) == 0) { m->entries[i].val = val; return; }
    if (m->len == m->cap) { m->cap = m->cap ? m->cap * 2 : 4; m->entries = realloc(m->entries, m->cap * sizeof(Map_str_strEntry)); }
    m->entries[m->len].key = k; m->entries[m->len].val = val; m->len++;
}
static char** Map_str_str_get(Map_str_str* m, char* k) {
    for (size_t i = 0; i < m->len; i++) if (strcmp(m->entries[i].key, k) == 0) return &m->entries[i].val;
    return NULL;
}
static bool Map_str_str_contains(Map_str_str* m, char* k) { return Map_str_str_get(m, k) != NULL; }

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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

struct vbr_http_buf { char* data; size_t len; };
static size_t vbr_http__write(char* ptr, size_t size, size_t nmemb, void* userdata) {
    size_t n = size * nmemb;
    struct vbr_http_buf* b = (struct vbr_http_buf*)userdata;
    char* d = (char*)realloc(b->data, b->len + n + 1);
    if (!d) return 0;
    b->data = d;
    memcpy(b->data + b->len, ptr, n);
    b->len += n;
    b->data[b->len] = '\0';
    return n;
}
static Result_str_str vbr_http__perform(CURL* curl, struct vbr_http_buf* buf, struct curl_slist* hdrs) {
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, vbr_http__write);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, buf);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 60L);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    CURLcode rc = curl_easy_perform(curl);
    if (hdrs) curl_slist_free_all(hdrs);
    if (rc != CURLE_OK) {
        char* e = vbr_dup(curl_easy_strerror(rc));
        free(buf->data);
        curl_easy_cleanup(curl);
        return (Result_str_str){ .is_ok = false, .err = e };
    }
    curl_easy_cleanup(curl);
    return (Result_str_str){ .is_ok = true, .ok = buf->data ? buf->data : vbr_dup("") };
}
static Result_str_str vbr_http_get(char* url) {
    CURL* curl = curl_easy_init();
    if (!curl) return (Result_str_str){ .is_ok = false, .err = vbr_dup("curl init failed") };
    struct vbr_http_buf buf = {0};
    curl_easy_setopt(curl, CURLOPT_URL, url);
    return vbr_http__perform(curl, &buf, NULL);
}
static Result_str_str vbr_http_post(char* url, char* body, Map_str_str headers) {
    CURL* curl = curl_easy_init();
    if (!curl) return (Result_str_str){ .is_ok = false, .err = vbr_dup("curl init failed") };
    struct vbr_http_buf buf = {0};
    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body);
    struct curl_slist* hdrs = NULL;
    for (size_t i = 0; i < headers.len; i++) {
        char* k = headers.entries[i].key;
        char* v = headers.entries[i].val;
        char* line = (char*)malloc(strlen(k) + strlen(v) + 3);
        strcpy(line, k); strcat(line, ": "); strcat(line, v);
        hdrs = curl_slist_append(hdrs, line);
        free(line);
    }
    if (hdrs) curl_easy_setopt(curl, CURLOPT_HTTPHEADER, hdrs);
    return vbr_http__perform(curl, &buf, hdrs);
}

int main(void) {
    char* key = vbr_dup("sk-demo-key");
    char* body = vbr_dup("{\"model\": \"demo\", \"prompt\": \"hello\"}");
    Map_str_str headers = {0};
    Map_str_str_insert(&headers, "Authorization", vbr_concat("Bearer ", key));
    Map_str_str_insert(&headers, "Content-Type", "application/json");
    char* reply = NULL;
    Result_str_str _t0 = vbr_http_post("https://api.example.com/v1/complete", body, headers);
    if (!_t0.is_ok) {
        char* message = _t0.err;
        printf("%s\n", vbr_concat("request failed: ", message));
        return 0;
    } else {
        reply = _t0.ok;
    }
    printf("%s\n", vbr_concat(vbr_concat("got ", vbr_from_ll((long long)strlen(reply))), " bytes"));
    return 0;
}
