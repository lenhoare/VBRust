// FileSystem from the standard library — write a file, read it back, check it
// exists, then delete it. Fallible calls propagate automatically; a missing file
// prints and exits from Main. Backed by stdio + POSIX; the same program
// transpiles to Rust, Python and C.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <errno.h>
#include <sys/stat.h>

typedef struct { bool is_ok; char* err; } Result_unit_str;
static void Result_unit_str_unwrap(Result_unit_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } }

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
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

static Result_unit_str vbr_fs_delete(char* path) {
    if (remove(path) != 0) return (Result_unit_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    return (Result_unit_str){ .is_ok = true };
}
static bool vbr_fs_exists(char* path) {
    struct stat st;
    return stat(path, &st) == 0 && S_ISREG(st.st_mode);
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

int main(void) {
    Result_unit_str _t0 = vbr_fs_write("vbr_fs_demo.txt", "Hello from Bust");
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    (void)0;
    Result_str_str _t1 = vbr_fs_read("vbr_fs_demo.txt");
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    char* text = _t1.ok;
    printf("%s\n", vbr_concat("file says:    ", text));
    printf("%s\n", vbr_concat("exists:       ", vbr_from_bool(vbr_fs_exists("vbr_fs_demo.txt"))));
    Result_unit_str _t2 = vbr_fs_delete("vbr_fs_demo.txt");
    if (!_t2.is_ok) { fprintf(stderr, "Error: %s\n", _t2.err); return 1; }
    (void)0;
    printf("%s\n", vbr_concat("after delete: ", vbr_from_bool(vbr_fs_exists("vbr_fs_demo.txt"))));
    return 0;
}
