/* Compile a VBR-generated C program with libtcc and run main() in-process.
 *
 * Host: system headers + the TinyCC we built in third_party/tcc-host.
 * Android: we strip the generated #includes (Bionic headers aren't on the
 * device) and prepend a small prelude of libc prototypes; TinyCC binds them
 * to the libc already loaded in the app process.
 */

#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#ifdef __ANDROID__
#include <dlfcn.h>
#endif

#include "libtcc.h"
#include "tcc_run.h"

#ifdef __ANDROID__
/* TinyCC's JIT calls this after writing machine code. Bionic doesn't export
 * it; the NDK compiler-rt usually would, but we link a Rust cdylib. */
void __clear_cache(void *beg, void *end) {
    __builtin___clear_cache((char *)beg, (char *)end);
}

/* Bind the prelude to libc/libm already loaded in this process. Android 10+
 * keeps those .so files in an APEX; tcc_add_library("c") can't find them. */
static void bind_android_libc(TCCState *s) {
    tcc_add_symbol(s, "malloc", (void *)malloc);
    tcc_add_symbol(s, "realloc", (void *)realloc);
    tcc_add_symbol(s, "free", (void *)free);
    tcc_add_symbol(s, "exit", (void *)exit);
    tcc_add_symbol(s, "strlen", (void *)strlen);
    tcc_add_symbol(s, "strcpy", (void *)strcpy);
    tcc_add_symbol(s, "strcat", (void *)strcat);
    tcc_add_symbol(s, "memcpy", (void *)memcpy);
    tcc_add_symbol(s, "strcmp", (void *)strcmp);
    tcc_add_symbol(s, "printf", (void *)printf);
    tcc_add_symbol(s, "snprintf", (void *)snprintf);
    tcc_add_symbol(s, "fprintf", (void *)fprintf);
    {
        /* Bionic's `stderr` is a macro, not an addressable global. */
        static void *vbr_stderr;
        vbr_stderr = (void *)stderr;
        tcc_add_symbol(s, "stderr", (void *)&vbr_stderr);
    }
    tcc_add_symbol(s, "strtod", (void *)strtod);
    tcc_add_symbol(s, "strtof", (void *)strtof);
    tcc_add_symbol(s, "pow", (void *)pow);
    tcc_add_symbol(s, "sqrt", (void *)sqrt);
    tcc_add_symbol(s, "floor", (void *)floor);
    tcc_add_symbol(s, "round", (void *)round);
    tcc_add_symbol(s, "fabs", (void *)fabs);
    tcc_add_symbol(s, "sin", (void *)sin);
    tcc_add_symbol(s, "cos", (void *)cos);
    tcc_add_symbol(s, "exp", (void *)exp);
    tcc_add_symbol(s, "log", (void *)log);
}
#endif

struct errbuf {
    char *s;
    size_t n;
};

static void on_err(void *opaque, const char *msg) {
    struct errbuf *e = opaque;
    size_t m = strlen(msg);
    char *n = (char *)realloc(e->s, e->n + m + 2);
    if (!n) return;
    e->s = n;
    memcpy(e->s + e->n, msg, m);
    e->n += m;
    e->s[e->n++] = '\n';
    e->s[e->n] = '\0';
}

static char *dup_str(const char *s) {
    size_t n = strlen(s);
    char *d = (char *)malloc(n + 1);
    if (d) memcpy(d, s, n + 1);
    return d;
}

/* Enough declarations for VBR's core C runtime (scalars, strings, maths).
 * FILE is unused here; fprintf(stderr, …) in Unwrap is typed as void*. */
static const char PRELUDE[] =
    "typedef unsigned long size_t;\n"
    "typedef _Bool bool;\n"
    "#define true 1\n"
    "#define false 0\n"
    "#define NULL ((void*)0)\n"
    "void *malloc(size_t);\n"
    "void *realloc(void *, size_t);\n"
    "void free(void *);\n"
    "void exit(int);\n"
    "size_t strlen(const char *);\n"
    "char *strcpy(char *, const char *);\n"
    "char *strcat(char *, const char *);\n"
    "void *memcpy(void *, const void *, size_t);\n"
    "int strcmp(const char *, const char *);\n"
    "int printf(const char *, ...);\n"
    "int snprintf(char *, size_t, const char *, ...);\n"
    "int fprintf(void *, const char *, ...);\n"
    "extern void *stderr;\n"
    "double strtod(const char *, char **);\n"
    "float strtof(const char *, char **);\n"
    "double pow(double, double);\n"
    "double sqrt(double);\n"
    "double floor(double);\n"
    "double round(double);\n"
    "double fabs(double);\n"
    "double sin(double);\n"
    "double cos(double);\n"
    "double exp(double);\n"
    "double log(double);\n"
    "\n";

/* Drop #include / feature-test lines so we don't need a sysroot on device. */
static char *strip_includes(const char *src) {
    size_t cap = strlen(src) + 1;
    char *out = (char *)malloc(cap);
    if (!out) return NULL;
    const char *p = src;
    char *o = out;
    while (*p) {
        const char *eol = strchr(p, '\n');
        size_t n = eol ? (size_t)(eol - p) + 1 : strlen(p);
        int drop = 0;
        const char *q = p;
        while (*q == ' ' || *q == '\t') q++;
        if (*q == '#') {
            const char *d = q + 1;
            while (*d == ' ' || *d == '\t') d++;
            if (strncmp(d, "include", 7) == 0) drop = 1;
            if (strncmp(d, "define _GNU_SOURCE", 18) == 0) drop = 1;
        }
        if (!drop) {
            memcpy(o, p, n);
            o += n;
        }
        p += n;
        if (!eol) break;
    }
    *o = '\0';
    return out;
}

static char *read_fd(int fd) {
    size_t cap = 4096, n = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) return dup_str("");
    for (;;) {
        if (n + 1024 > cap) {
            cap *= 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) break;
            buf = nb;
        }
        ssize_t r = read(fd, buf + n, cap - n - 1);
        if (r <= 0) break;
        n += (size_t)r;
    }
    buf[n] = '\0';
    return buf;
}

void vbr_tcc_result_free(VbrTccResult *r) {
    if (!r) return;
    free(r->stdout_text);
    free(r->stderr_text);
    r->stdout_text = NULL;
    r->stderr_text = NULL;
}

int vbr_tcc_run(const char *c_source, const char *tccdir, int use_prelude,
                VbrTccResult *out) {
    memset(out, 0, sizeof(*out));
    out->stdout_text = dup_str("");
    out->stderr_text = dup_str("");

    TCCState *s = tcc_new();
    if (!s) {
        free(out->stderr_text);
        out->stderr_text = dup_str("TinyCC failed to start (tcc_new).");
        return -1;
    }

    struct errbuf err = {0};
    tcc_set_error_func(s, &err, on_err);

    if (tccdir && tccdir[0]) {
        tcc_set_lib_path(s, tccdir);
    }

    tcc_set_output_type(s, TCC_OUTPUT_MEMORY);
    if (use_prelude) {
        /* No sysroot on the phone — prototypes come from the prelude. */
        tcc_set_options(s, "-nostdlib");
    }
#ifndef __ANDROID__
    tcc_add_library(s, "c");
    tcc_add_library(s, "m");
#endif

    char *body = NULL;
    const char *compile_src = c_source;
    if (use_prelude) {
        char *stripped = strip_includes(c_source);
        if (!stripped) {
            tcc_delete(s);
            free(out->stderr_text);
            out->stderr_text = dup_str("out of memory");
            return -1;
        }
        size_t n = strlen(PRELUDE) + strlen(stripped) + 1;
        body = (char *)malloc(n);
        if (!body) {
            free(stripped);
            tcc_delete(s);
            free(out->stderr_text);
            out->stderr_text = dup_str("out of memory");
            return -1;
        }
        strcpy(body, PRELUDE);
        strcat(body, stripped);
        free(stripped);
        compile_src = body;
    }

    if (tcc_compile_string(s, compile_src) == -1) {
        free(body);
        tcc_delete(s);
        free(out->stderr_text);
        out->stderr_text = err.s ? err.s : dup_str("TinyCC rejected the generated C.");
        err.s = NULL;
        return -1;
    }
    free(body);
#ifdef __ANDROID__
    bind_android_libc(s);
#endif

    int pipefd[2];
    if (pipe(pipefd) != 0) {
        tcc_delete(s);
        free(err.s);
        free(out->stderr_text);
        out->stderr_text = dup_str("Could not capture program output.");
        return -1;
    }
#ifdef F_SETPIPE_SZ
    fcntl(pipefd[1], F_SETPIPE_SZ, 1 << 20);
#endif

    fflush(stdout);
    fflush(stderr);
    int saved_out = dup(STDOUT_FILENO);
    int saved_err = dup(STDERR_FILENO);
    dup2(pipefd[1], STDOUT_FILENO);
    dup2(pipefd[1], STDERR_FILENO);
    close(pipefd[1]);

    int code = 0;
#ifdef __ANDROID__
    {
        if (tcc_relocate(s) < 0) {
            if (saved_out >= 0) { dup2(saved_out, STDOUT_FILENO); close(saved_out); }
            if (saved_err >= 0) { dup2(saved_err, STDERR_FILENO); close(saved_err); }
            close(pipefd[0]);
            tcc_delete(s);
            free(out->stderr_text);
            out->stderr_text = err.s ? err.s : dup_str("TinyCC relocate failed (no executable memory?).");
            err.s = NULL;
            return -1;
        }
        int (*prog_main)(int, char **) = tcc_get_symbol(s, "main");
        if (!prog_main) {
            if (saved_out >= 0) { dup2(saved_out, STDOUT_FILENO); close(saved_out); }
            if (saved_err >= 0) { dup2(saved_err, STDERR_FILENO); close(saved_err); }
            close(pipefd[0]);
            tcc_delete(s);
            free(out->stderr_text);
            out->stderr_text = dup_str("TinyCC produced no main()");
            return -1;
        }
        code = prog_main(0, NULL);
    }
#else
    code = tcc_run(s, 0, NULL);
#endif

    fflush(stdout);
    fflush(stderr);
    if (saved_out >= 0) {
        dup2(saved_out, STDOUT_FILENO);
        close(saved_out);
    }
    if (saved_err >= 0) {
        dup2(saved_err, STDERR_FILENO);
        close(saved_err);
    }

    free(out->stdout_text);
    out->stdout_text = read_fd(pipefd[0]);
    close(pipefd[0]);
    out->exit_code = code;
    out->ok = 1;
    if (err.s) {
        free(out->stderr_text);
        out->stderr_text = err.s;
        err.s = NULL;
    }
    if (s) tcc_delete(s);
    return 0;
}
