// Standard library — file I/O and regex. Calls translate `.` to `::`.

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <errno.h>
#include <sys/stat.h>
#include <regex.h>

typedef struct { bool is_ok; char* err; } Result_unit_str;
static void Result_unit_str_unwrap(Result_unit_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } }

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

static Result_unit_str vbr_fs_delete(char* path) {
    if (remove(path) != 0) return (Result_unit_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    return (Result_unit_str){ .is_ok = true };
}
static Result_str_str vbr_fs_read(char* path) {
    FILE* f = fopen(path, "rb");
    if (!f) return (Result_str_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc((size_t)n + 1);
    size_t got = fread(buf, 1, (size_t)n, f);
    buf[got] = '\0';
    fclose(f);
    return (Result_str_str){ .is_ok = true, .ok = buf };
}
static Result_unit_str vbr_fs_write(char* path, char* contents) {
    FILE* f = fopen(path, "wb");
    if (!f) return (Result_unit_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    fwrite(contents, 1, strlen(contents), f);
    fclose(f);
    return (Result_unit_str){ .is_ok = true };
}
static char* vbr_regex_posix(char* pat) {
    char* out = (char*)malloc(strlen(pat) * 12 + 1);
    char* o = out;
    for (char* p = pat; *p; p++) {
        if (*p == '\\' && p[1]) {
            const char* rep = NULL;
            switch (p[1]) {
                case 's': rep = "[[:space:]]"; break;
                case 'S': rep = "[^[:space:]]"; break;
                case 'd': rep = "[[:digit:]]"; break;
                case 'D': rep = "[^[:digit:]]"; break;
                case 'w': rep = "[[:alnum:]_]"; break;
                case 'W': rep = "[^[:alnum:]_]"; break;
            }
            if (rep) { strcpy(o, rep); o += strlen(rep); p++; continue; }
        }
        *o++ = *p;
    }
    *o = '\0';
    return out;
}
static Result_str_str vbr_regex_replaceall(char* pattern, char* text, char* replacement) {
    char* pat = vbr_regex_posix(pattern);
    regex_t re;
    if (regcomp(&re, pat, REG_EXTENDED) != 0) {
        free(pat);
        return (Result_str_str){ .is_ok = false, .err = vbr_dup("invalid regex") };
    }
    free(pat);
    size_t cap = 64, len = 0, rlen = strlen(replacement);
    char* out = (char*)malloc(cap);
    const char* cur = text;
    int not_bol = 0;
    regmatch_t m;
    while (regexec(&re, cur, 1, &m, not_bol ? REG_NOTBOL : 0) == 0) {
        size_t pre = (size_t)m.rm_so;
        while (len + pre + rlen + 2 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
        memcpy(out + len, cur, pre); len += pre;
        memcpy(out + len, replacement, rlen); len += rlen;
        size_t adv = (size_t)m.rm_eo;
        if (m.rm_eo == m.rm_so) {
            if (cur[m.rm_eo] == '\0') break;
            out[len++] = cur[m.rm_eo];
            adv = (size_t)m.rm_eo + 1;
        }
        cur += adv;
        not_bol = 1;
        if (*cur == '\0') break;
    }
    size_t tail = strlen(cur);
    while (len + tail + 1 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
    memcpy(out + len, cur, tail); len += tail;
    out[len] = '\0';
    regfree(&re);
    return (Result_str_str){ .is_ok = true, .ok = out };
}

int main(void) {
    Result_unit_str _t0 = vbr_fs_write("greeting.txt", "Hello   from   Bust");
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    (void)0;
    Result_str_str _t1 = vbr_fs_read("greeting.txt");
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    char* text = _t1.ok;
    printf("%s\n", vbr_concat("file says: ", text));
    Result_str_str _t2 = vbr_regex_replaceall("\\s+", text, " ");
    if (!_t2.is_ok) { fprintf(stderr, "Error: %s\n", _t2.err); return 1; }
    char* cleaned = _t2.ok;
    printf("%s\n", vbr_concat("cleaned:   ", cleaned));
    Result_unit_str _t3 = vbr_fs_delete("greeting.txt");
    if (!_t3.is_ok) { fprintf(stderr, "Error: %s\n", _t3.err); return 1; }
    (void)0;
    return 0;
}
