// DateTime from the standard library — parse a fixed moment, then read, format
// and shift it. (Uses Parse, not Now, so the output is deterministic.)

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <time.h>

typedef struct tm DateTime;

typedef struct { bool is_ok; DateTime ok; char* err; } Result_DateTime_str;
static DateTime Result_DateTime_str_unwrap(Result_DateTime_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

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

static Result_DateTime_str vbr_datetime_parse(char* text, char* pattern) {
    struct tm tm = {0};
    if (strptime(text, pattern, &tm) == NULL)
        return (Result_DateTime_str){ .is_ok = false, .err = vbr_dup("could not parse date") };
    return (Result_DateTime_str){ .is_ok = true, .ok = tm };
}
static DateTime vbr_datetime_now(void) {
    time_t t = time(NULL);
    struct tm r;
    localtime_r(&t, &r);
    return r;
}
static long long vbr_datetime_year(DateTime* d) { return d->tm_year + 1900; }
static long long vbr_datetime_month(DateTime* d) { return d->tm_mon + 1; }
static long long vbr_datetime_day(DateTime* d) { return d->tm_mday; }
static char* vbr_datetime_format(DateTime* d, char* pattern) {
    char buf[256];
    strftime(buf, sizeof buf, pattern, d);
    return vbr_dup(buf);
}
static DateTime vbr_datetime_shift(DateTime* d, long long seconds) {
    struct tm t = *d;
    time_t s = timegm(&t) + seconds;
    struct tm r;
    gmtime_r(&s, &r);
    return r;
}
static DateTime vbr_datetime_adddays(DateTime* d, long long days) { return vbr_datetime_shift(d, days * 86400); }
static DateTime vbr_datetime_addhours(DateTime* d, long long hours) { return vbr_datetime_shift(d, hours * 3600); }
static DateTime vbr_datetime_addminutes(DateTime* d, long long mins) { return vbr_datetime_shift(d, mins * 60); }

int main(void) {
    DateTime d = Result_DateTime_str_unwrap(vbr_datetime_parse("2026-07-24 09:30:00", "%Y-%m-%d %H:%M:%S"));
    printf("%s\n", vbr_concat("year:  ", vbr_from_ll(vbr_datetime_year(&d))));
    printf("%s\n", vbr_concat("month: ", vbr_from_ll(vbr_datetime_month(&d))));
    printf("%s\n", vbr_concat("day:   ", vbr_from_ll(vbr_datetime_day(&d))));
    printf("%s\n", vbr_concat("iso:   ", vbr_datetime_format(&d, "%Y-%m-%d")));
    DateTime later = vbr_datetime_adddays(&d, 10);
    printf("%s\n", vbr_concat("in 10 days: ", vbr_datetime_format(&later, "%Y-%m-%d")));
    DateTime soon = vbr_datetime_addhours(&d, 5);
    printf("%s\n", vbr_concat("in 5 hours: ", vbr_datetime_format(&soon, "%H:%M")));
    return 0;
}
