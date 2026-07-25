// Shell — VB6's `Shell`, grown up. Two verbs:
// Shell.Run(cmd)   — run through the system shell, WAIT, capture the output:
// Ok(stdout) on success, Err(stderr) on a nonzero exit.
// Shell.Start(cmd) — launch and DON'T wait (VB6's actual Shell semantics):
// you get a Process handle to check on or stop.
// Pipes and PATH work — the command line goes through sh -c / cmd /C.
// A background child: start it, peek at it, stop it. `Kill` on an already-dead
// process is a harmless no-op; `Wait` returns the exit code (-1 after a kill).

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <errno.h>
#include <sys/wait.h>
#include <signal.h>
#include <unistd.h>

typedef struct { int pid; int reaped; long long code; } Process;

typedef struct { bool is_ok; long long ok; char* err; } Result_longlong_str;
static long long Result_longlong_str_unwrap(Result_longlong_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; char* ok; char* err; } Result_str_str;
static char* Result_str_str_unwrap(Result_str_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

typedef struct { bool is_ok; Process ok; char* err; } Result_Process_str;
static Process Result_Process_str_unwrap(Result_Process_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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

static Result_str_str vbr_shell_run(char* cmd) {
    FILE* p = popen(cmd, "r");
    if (!p) return (Result_str_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    size_t cap = 256, len = 0;
    char* out = (char*)malloc(cap);
    int c;
    while ((c = fgetc(p)) != EOF) {
        if (len + 2 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
        out[len++] = (char)c;
    }
    out[len] = '\0';
    while (len > 0 && (out[len - 1] == '\n' || out[len - 1] == '\r' || out[len - 1] == ' ' || out[len - 1] == '\t'))
        out[--len] = '\0';
    int status = pclose(p);
    int code = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
    if (code == 0) return (Result_str_str){ .is_ok = true, .ok = out };
    char msg[64];
    snprintf(msg, sizeof msg, "command failed with code %d", code);
    free(out);
    return (Result_str_str){ .is_ok = false, .err = vbr_dup(msg) };
}
static Result_Process_str vbr_shell_start(char* cmd) {
    pid_t pid = fork();
    if (pid < 0) return (Result_Process_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    if (pid == 0) { execl("/bin/sh", "sh", "-c", cmd, (char*)NULL); _exit(127); }
    return (Result_Process_str){ .is_ok = true, .ok = (Process){ .pid = pid, .reaped = 0, .code = 0 } };
}
static void vbr_process_kill(Process* p) {
    if (p->reaped) return;
    kill(p->pid, SIGKILL);
    int st; waitpid(p->pid, &st, 0);
    p->reaped = 1; p->code = -1;
}
static bool vbr_process_isrunning(Process* p) {
    if (p->reaped) return false;
    int st;
    pid_t r = waitpid(p->pid, &st, WNOHANG);
    if (r == 0) return true;
    p->reaped = 1;
    p->code = WIFEXITED(st) ? WEXITSTATUS(st) : -1;
    return false;
}
static long long vbr_process_wait(Process* p) {
    if (p->reaped) return p->code;
    int st; waitpid(p->pid, &st, 0);
    p->reaped = 1;
    p->code = WIFEXITED(st) ? WEXITSTATUS(st) : -1;
    return p->code;
}

Result_longlong_str runchild(void);

int main(void) {
    Result_str_str _m0 = vbr_shell_run("echo hello from VBR");
    if (_m0.is_ok) {
        char* output = _m0.ok;
        printf("%s\n", vbr_concat("said: ", output));
    } else {
        char* why = _m0.err;
        printf("%s\n", vbr_concat("echo failed: ", why));
    }
    Result_str_str _m1 = vbr_shell_run("ls /vbr/definitely/missing");
    if (_m1.is_ok) {
        char* output = _m1.ok;
        printf("%s\n", output);
    } else {
        printf("%s\n", "as expected, that failed");
    }
    Result_longlong_str _m2 = runchild();
    if (_m2.is_ok) {
        long long code = _m2.ok;
        printf("%s\n", vbr_concat("child finished with exit code ", vbr_from_ll(code)));
    } else {
        char* why = _m2.err;
        printf("%s\n", vbr_concat("child failed: ", why));
    }
    return 0;
}

Result_longlong_str runchild(void) {
    Result_Process_str _t0 = vbr_shell_start("sleep 2");
    if (!_t0.is_ok) return (Result_longlong_str){ .is_ok = false, .err = _t0.err };
    Process child = _t0.ok;
    usleep((100) * 1000);
    // VB6's kernel32 Sleep, no Declare needed (milliseconds)
    printf("%s\n", vbr_concat("running: ", vbr_from_bool(vbr_process_isrunning(&child))));
    vbr_process_kill(&child);
    printf("%s\n", vbr_concat("after kill: ", vbr_from_bool(vbr_process_isrunning(&child))));
    return (Result_longlong_str){ .is_ok = true, .ok = vbr_process_wait(&child) };
}
